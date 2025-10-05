use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, exit};
use std::time::Duration;
use std::thread;

use directories::{ProjectDirs, UserDirs};
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

const PROFILE_FILE_NAME: &str = "profiles.json";
const TEMP_PROFILE_PREFIX: &str = "__temp_";
const ELEVATION_CLEANUP_FLAG: &str = "--cleanup-temp";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
enum ProcessPriority {
    Idle,
    BelowNormal,
    Normal,
    AboveNormal,
    High,
    Realtime,
}

impl ProcessPriority {
    fn display_name(&self) -> &str {
        match self {
            Self::Idle => "Idle",
            Self::BelowNormal => "Below Normal",
            Self::Normal => "Normal",
            Self::AboveNormal => "Above Normal",
            Self::High => "High",
            Self::Realtime => "Realtime",
        }
    }

    #[cfg(target_os = "windows")]
    fn to_windows_class(&self) -> u32 {
        use windows_sys::Win32::System::Threading::*;
        match self {
            Self::Idle => IDLE_PRIORITY_CLASS,
            Self::BelowNormal => BELOW_NORMAL_PRIORITY_CLASS,
            Self::Normal => NORMAL_PRIORITY_CLASS,
            Self::AboveNormal => ABOVE_NORMAL_PRIORITY_CLASS,
            Self::High => HIGH_PRIORITY_CLASS,
            Self::Realtime => REALTIME_PRIORITY_CLASS,
        }
    }

    #[cfg(target_os = "windows")]
    fn requires_elevation(&self) -> bool {
        matches!(self, Self::High | Self::Realtime)
    }

    #[cfg(target_os = "linux")]
    fn to_nice_value(&self) -> &str {
        match self {
            Self::Idle => "19",
            Self::BelowNormal => "10",
            Self::Normal => "0",
            Self::AboveNormal => "-5",
            Self::High => "-10",
            Self::Realtime => "-20",
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Profile {
    path: PathBuf,
    cpus: Vec<usize>,
    #[serde(default)]
    priority: Option<ProcessPriority>,
    #[serde(default)]
    retry_attempts: Option<usize>,
}

type Profiles = HashMap<String, Profile>;

fn get_profile_path() -> Result<PathBuf> {
    let proj_dirs = ProjectDirs::from("rs", "affinity", "AffinityRs")
        .context("Could not find a valid home directory to store profiles")?;

    let config_dir = proj_dirs.config_dir();
    std::fs::create_dir_all(config_dir).context("Failed to create config directory")?;

    let mut config_file_path = config_dir.to_path_buf();
    config_file_path.push(PROFILE_FILE_NAME);
    Ok(config_file_path)
}

fn load_profiles() -> Result<Profiles> {
    let profile_path = get_profile_path()?;
    
    if !profile_path.exists() {
        return Ok(Profiles::new());
    }

    let data = std::fs::read_to_string(&profile_path)
        .context("Failed to read profiles file")?;
    
    serde_json::from_str(&data)
        .context("Failed to parse profiles JSON")
}

fn save_profiles(profiles: &Profiles) -> Result<()> {
    let profile_path = get_profile_path()?;
    let data = serde_json::to_string_pretty(profiles)
        .context("Failed to serialize profiles")?;
    std::fs::write(profile_path, data)
        .context("Failed to write profiles to disk")?;
    Ok(())
}

fn pause_before_exit() {
    print!("\nPress Enter to exit...");
    let _ = io::stdout().flush();
    let mut dummy = String::new();
    let _ = io::stdin().read_line(&mut dummy);
}

fn read_line(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush().context("Failed to flush stdout")?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;
    Ok(input.trim().to_string())
}

fn get_cpu_input() -> Result<Vec<usize>> {
    loop {
        let input = read_line("Enter CPU cores (comma-separated, e.g., 0,1,2,3): ")?;
        let trimmed = input.trim();
        
        if trimmed.is_empty() {
            eprintln!("Error: CPU cores cannot be empty.");
            continue;
        }
        
        let is_valid = trimmed
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c.is_whitespace());
        
        if !is_valid {
            eprintln!("Error: only numbers, commas, and spaces allowed.");
            continue;
        }
        
        let cpus: Vec<usize> = trimmed
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        
        if cpus.is_empty() {
            eprintln!("Error: no valid cores provided.");
            continue;
        }
        
        return Ok(cpus);
    }
}

fn get_priority_input() -> Result<Option<ProcessPriority>> {
    println!("\nProcess Priority options:");
    println!("  1. Idle");
    println!("  2. Below Normal");
    println!("  3. Normal [default]");
    println!("  4. Above Normal");
    
    #[cfg(target_os = "windows")]
    {
        println!("  5. High [requires admin]");
        println!("  6. Realtime [requires admin - WARNING: Can freeze your system!]");
    }
    
    #[cfg(not(target_os = "windows"))]
    {
        println!("  5. High [may require sudo]");
        println!("  6. Realtime [may require sudo - WARNING: Can freeze your system!]");
    }
    
    let input = read_line("Enter priority (1-6, or press Enter for Normal): ")?;
    let trimmed = input.trim();
    
    if trimmed.is_empty() {
        return Ok(None);
    }
    
    let priority = match trimmed {
        "1" => ProcessPriority::Idle,
        "2" => ProcessPriority::BelowNormal,
        "3" => ProcessPriority::Normal,
        "4" => ProcessPriority::AboveNormal,
        "5" => ProcessPriority::High,
        "6" => {
            println!("\nWARNING: Realtime priority can make your system unresponsive!");
            println!("Only use this if you understand the risks.");
            ProcessPriority::Realtime
        },
        _ => {
            eprintln!("Invalid selection, using Normal priority");
            ProcessPriority::Normal
        }
    };
    
    Ok(Some(priority))
}

#[cfg(target_os = "windows")]
fn is_elevated() -> bool {
    use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
    use windows_sys::Win32::Foundation::CloseHandle;
    
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return false;
        }
        
        let mut elevation = TOKEN_ELEVATION { TokenIsElevated: 0 };
        let mut return_length: u32 = 0;
        
        let result = GetTokenInformation(
            token,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut return_length,
        );
        
        CloseHandle(token);
        
        result != 0 && elevation.TokenIsElevated != 0
    }
}

#[cfg(target_os = "windows")]
fn relaunch_elevated(profile_name: &str, args: &[String]) -> Result<()> {
    use windows_sys::Win32::UI::Shell::ShellExecuteW;
    use windows_sys::Win32::Foundation::HWND;
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
    
    let current_exe = std::env::current_exe()
        .context("Failed to get current executable path")?;
    let current_exe_str = current_exe.to_str()
        .context("Executable path contains invalid UTF-8")?;
    
    println!("\nAdministrator privileges required for this priority level.");
    println!("Requesting elevation...\n");
    
    // Build parameters: profile_name + cleanup flag + any additional args
    let mut params = vec![profile_name.to_string(), ELEVATION_CLEANUP_FLAG.to_string()];
    params.extend_from_slice(args);
    let params_str = params.join(" ");
    
    unsafe {
        // Convert strings to wide strings for Windows API
        let operation: Vec<u16> = "runas\0".encode_utf16().collect();
        let file: Vec<u16> = current_exe_str.encode_utf16().chain(Some(0)).collect();
        let parameters: Vec<u16> = params_str.encode_utf16().chain(Some(0)).collect();
        
        let result = ShellExecuteW(
            0 as HWND,
            operation.as_ptr(),
            file.as_ptr(),
            parameters.as_ptr(),
            std::ptr::null(),
            SW_SHOWNORMAL as i32,
        );
        
        // ShellExecuteW returns > 32 on success
        let result_code = result as isize;
        if result_code > 32 {
            println!("Elevated process launched successfully.");
            println!("This window will now close.\n");
            thread::sleep(Duration::from_millis(500));
            Ok(())
        } else {
            // Handle specific error codes
            match result_code {
                0 => bail!("Out of memory or resources"),
                2 => bail!("File not found"),
                3 => bail!("Path not found"),
                5 => bail!("Access denied - User may have cancelled UAC prompt"),
                8 => bail!("Out of memory"),
                31 => bail!("No application associated with this file type"),
                _ => bail!("ShellExecuteW failed with error code: {}", result_code),
            }
        }
    }
}

fn validate_profile(profile: &Profile) -> Result<()> {
    if !profile.path.exists() {
        bail!(
            "Executable not found: {}\nThe file may have been moved or deleted.",
            profile.path.display()
        );
    }
    
    if profile.cpus.is_empty() {
        bail!("Profile has no CPU cores configured");
    }
    
    // Check if CPU indices are reasonable
    let max_cpu = profile.cpus.iter().max().unwrap();
    let system_cpu_count = num_cpus::get();
    
    if *max_cpu >= system_cpu_count {
        eprintln!(
            "Warning: Profile references CPU {}, but system only has {} logical CPUs",
            max_cpu, system_cpu_count
        );
        eprintln!("Some CPU assignments may be ignored by the OS.");
    }
    
    Ok(())
}

fn launch_with_retry<F>(
    attempts: usize,
    initial_delay_ms: u64,
    mut operation: F
) -> Result<bool>
where
    F: FnMut(usize) -> Result<bool>,
{
    for attempt in 1..=attempts {
        let delay = if attempt == 1 {
            initial_delay_ms
        } else {
            // Exponential backoff with cap at 1000ms
            (initial_delay_ms * 2_u64.pow((attempt - 1) as u32)).min(1000)
        };
        
        thread::sleep(Duration::from_millis(delay));
        
        match operation(attempt) {
            Ok(true) => return Ok(true),  // Success
            Ok(false) => continue,         // Retry
            Err(e) => {
                if attempt == attempts {
                    return Err(e);
                }
                eprintln!("Attempt {}/{} failed: {}. Retrying...", attempt, attempts, e);
            }
        }
    }
    
    Ok(false)
}

#[cfg(target_os = "linux")]
fn launch_profile_linux(profile: &Profile, args: &[String]) -> Result<()> {
    let cpu_str = profile
        .cpus
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(",");
    
    let mut cmd = Command::new("taskset");
    cmd.arg("-c").arg(&cpu_str);
    
    // Wrap with nice if priority is specified
    if let Some(ref priority) = profile.priority {
        let nice_value = priority.to_nice_value();
        let mut nice_cmd = Command::new("nice");
        nice_cmd
            .arg("-n")
            .arg(nice_value)
            .arg("taskset")
            .arg("-c")
            .arg(&cpu_str)
            .arg(&profile.path)
            .args(args);
        cmd = nice_cmd;
    } else {
        cmd.arg(&profile.path).args(args);
    }
    
    let child = cmd.spawn()
        .context("Failed to spawn process. Is 'taskset' installed?")?;
    
    println!("Process launched with PID: {}", child.id());
    println!("Program is running independently.\n");
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn launch_profile_windows(profile: &Profile, args: &[String]) -> Result<()> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        GetProcessAffinityMask, OpenProcess, PROCESS_QUERY_INFORMATION,
        PROCESS_SET_INFORMATION, SetProcessAffinityMask, SetPriorityClass,
        GetPriorityClass,
    };

    // Calculate affinity mask
    let mut affinity_mask: usize = 0;
    for &cpu in &profile.cpus {
        if cpu >= (std::mem::size_of::<usize>() * 8) {
            eprintln!(
                "Warning: CPU index {} is out of bounds for this system and will be ignored.",
                cpu
            );
            continue;
        }
        affinity_mask |= 1 << cpu;
    }
    
    if affinity_mask == 0 {
        bail!("No valid CPUs specified after validation");
    }

    let child = Command::new(&profile.path)
        .args(args)
        .spawn()
        .context("Failed to spawn process")?;
    
    let pid = child.id();
    println!("Process launched with PID: {}", pid);

    let retry_attempts = profile.retry_attempts.unwrap_or(5);
    let mut affinity_set = false;
    let mut priority_set = false;

    // Try multiple times to handle launcher -> game transitions
    let success = launch_with_retry(retry_attempts, 100, |attempt| {
        unsafe {
            let handle = OpenProcess(
                PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION,
                0,
                pid
            );
            
            if handle.is_null() {
                if attempt == retry_attempts {
                    eprintln!(
                        "Warning: Could not open process handle after {} attempts",
                        retry_attempts
                    );
                    eprintln!("The process may have exited or is protected.");
                    return Ok(false);
                }
                return Ok(false); // Retry
            }

            // Set CPU affinity
            if !affinity_set {
                let result = SetProcessAffinityMask(handle, affinity_mask);
                if result == 0 {
                    let err = std::io::Error::last_os_error();
                    CloseHandle(handle);
                    return Err(anyhow::anyhow!("Failed to set CPU affinity: {}", err));
                }
                
                // Verify affinity
                let mut process_affinity: usize = 0;
                let mut system_affinity: usize = 0;
                GetProcessAffinityMask(handle, &mut process_affinity, &mut system_affinity);
                
                if process_affinity == affinity_mask {
                    println!("CPU affinity set and verified: 0x{:X}", process_affinity);
                    affinity_set = true;
                } else {
                    println!(
                        "Warning: Affinity mismatch - Requested: 0x{:X}, Actual: 0x{:X}",
                        affinity_mask, process_affinity
                    );
                    affinity_set = true; // Don't retry if it was partially successful
                }
            }

            // Set process priority if specified
            if let Some(ref priority) = profile.priority {
                if !priority_set {
                    let priority_class = priority.to_windows_class();
                    let priority_result = SetPriorityClass(handle, priority_class);
                    
                    if priority_result == 0 {
                        let err = std::io::Error::last_os_error();
                        eprintln!("Failed to set process priority: {}", err);
                        
                        if priority.requires_elevation() && !is_elevated() {
                            eprintln!(
                                "Note: {} priority requires administrator privileges.",
                                priority.display_name()
                            );
                        }
                    } else {
                        // Verify priority after a short delay
                        thread::sleep(Duration::from_millis(100));
                        let actual_priority = GetPriorityClass(handle);
                        
                        if actual_priority == priority_class {
                            println!("Process priority set to: {}", priority.display_name());
                            priority_set = true;
                        } else if actual_priority == 0 {
                            eprintln!("Could not verify priority (GetPriorityClass failed)");
                            priority_set = true; // Don't keep retrying
                        } else {
                            println!(
                                "Note: Process reset its priority to a different value."
                            );
                            println!(
                                "This is normal for some applications (especially games with launchers)."
                            );
                            priority_set = true;
                        }
                    }
                }
            }

            CloseHandle(handle);
            
            // Check if process still exists for next attempt
            if attempt < retry_attempts && !affinity_set {
                thread::sleep(Duration::from_millis(200));
                let check_handle = OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid);
                if check_handle.is_null() {
                    println!(
                        "Note: Initial process (PID {}) has exited. This is likely a launcher.",
                        pid
                    );
                    println!("The actual game may be running with a different PID.");
                    println!(
                        "Tip: Try launching the game's actual .exe directly for better results."
                    );
                    return Ok(false); // Stop retrying
                }
                CloseHandle(check_handle);
            }
            
            // Success if affinity was set
            Ok(affinity_set)
        }
    })?;

    if !success {
        eprintln!("\nWarning: Could not fully configure the process.");
        eprintln!("The application may be using a launcher or may have restricted access.");
    }

    println!("\nProgram is running independently.\n");
    Ok(())
}

fn launch_profile(profile: &Profile, args: &[String]) -> Result<()> {
    println!(
        "\nLaunching: {}",
        profile.path.display()
    );
    println!("CPU affinity: {:?}", profile.cpus);
    
    if let Some(ref priority) = profile.priority {
        println!("Priority: {}", priority.display_name());
    }
    
    if !args.is_empty() {
        println!("Arguments: {:?}", args);
    }
    
    println!();

    #[cfg(target_os = "linux")]
    return launch_profile_linux(profile, args);

    #[cfg(target_os = "windows")]
    return launch_profile_windows(profile, args);
}

fn launch_or_exit(
    profile: &Profile,
    args: &[String],
    profile_name: Option<&str>,
    should_cleanup: bool,
) -> ! {
    // Validate profile before attempting launch
    if let Err(e) = validate_profile(profile) {
        eprintln!("Profile validation failed: {:#}", e);
        
        if profile_name.is_some() {
            eprintln!("\nWould you like to:");
            eprintln!("  1. Update the profile path");
            eprintln!("  2. Delete this profile");
            eprintln!("  3. Exit");
            
            if let Ok(choice) = read_line("Enter choice (1-3): ") {
                match choice.as_str() {
                    "1" => {
                        if let Ok(new_path) = read_line("Enter new executable path: ") {
                            let new_path = new_path.trim_matches('"');
                            if PathBuf::from(new_path).exists() {
                                if let Ok(mut profiles) = load_profiles() {
                                    if let Some(name) = profile_name {
                                        if let Some(p) = profiles.get_mut(name) {
                                            p.path = PathBuf::from(new_path);
                                            if save_profiles(&profiles).is_ok() {
                                                println!("Profile updated! Please run the command again.");
                                            }
                                        }
                                    }
                                }
                            } else {
                                eprintln!("Error: Path does not exist.");
                            }
                        }
                    }
                    "2" => {
                        if let (Ok(mut profiles), Some(name)) = (load_profiles(), profile_name) {
                            profiles.remove(name);
                            let _ = save_profiles(&profiles);
                            println!("Profile deleted.");
                        }
                    }
                    _ => {}
                }
            }
        }
        
        pause_before_exit();
        exit(1);
    }

    #[cfg(target_os = "windows")]
    {
        // Check if elevation is needed
        if let Some(ref priority) = profile.priority {
            if priority.requires_elevation() && !is_elevated() {
                // Create temp profile if needed
                let name = match profile_name {
                    Some(n) => n.to_string(),
                    None => {
                        // Create temporary profile for elevation
                        println!("\nNote: Using temporary profile for elevation.");
                        println!("Consider saving this profile if you'll use these settings again.\n");
                        
                        let temp_name = format!("{}{}",  TEMP_PROFILE_PREFIX, std::process::id());
                        
                        if let Ok(mut profiles) = load_profiles() {
                            profiles.insert(temp_name.clone(), profile.clone());
                            if let Err(e) = save_profiles(&profiles) {
                                eprintln!("Error: Failed to save temporary profile: {}", e);
                                pause_before_exit();
                                exit(1);
                            }
                        } else {
                            eprintln!("Error: Failed to load profiles for elevation");
                            pause_before_exit();
                            exit(1);
                        }
                        
                        temp_name
                    }
                };
                
                match relaunch_elevated(&name, args) {
                    Ok(_) => exit(0),
                    Err(e) => {
                        // Clean up temp profile if elevation failed
                        if name.starts_with(TEMP_PROFILE_PREFIX) {
                            if let Ok(mut profiles) = load_profiles() {
                                profiles.remove(&name);
                                let _ = save_profiles(&profiles);
                            }
                        }
                        
                        eprintln!("\nError requesting elevation: {:#}", e);
                        eprintln!("\nOptions:");
                        eprintln!("  1. Run this program as Administrator");
                        eprintln!("  2. Choose a lower priority (Normal or Above Normal)");
                        eprintln!("  3. Launch anyway with Normal priority");
                        
                        if let Ok(choice) = read_line("\nEnter choice (1-3): ") {
                            if choice == "3" {
                                println!("\nLaunching with Normal priority instead...");
                                let mut fallback_profile = profile.clone();
                                fallback_profile.priority = Some(ProcessPriority::Normal);
                                
                                match launch_profile(&fallback_profile, args) {
                                    Ok(_) => exit(0),
                                    Err(e) => {
                                        eprintln!("Error launching program: {:#}", e);
                                        pause_before_exit();
                                        exit(1);
                                    }
                                }
                            }
                        }
                        
                        pause_before_exit();
                        exit(1);
                    }
                }
            }
        }
    }
    
    // Launch the profile
    match launch_profile(profile, args) {
        Ok(_) => {
            // Clean up temp profile if requested
            if should_cleanup {
                if let Some(name) = profile_name {
                    if name.starts_with(TEMP_PROFILE_PREFIX) {
                        if let Ok(mut profiles) = load_profiles() {
                            profiles.remove(name);
                            let _ = save_profiles(&profiles);
                        }
                    }
                }
            }
            exit(0)
        }
        Err(e) => {
            eprintln!("Error launching program: {:#}", e);
            pause_before_exit();
            exit(1);
        }
    }
}

fn delete_profile(profiles: &mut Profiles, keyword: &str) -> Result<()> {
    if profiles.remove(keyword).is_some() {
        save_profiles(profiles)
            .context("Failed to save profiles after deletion")?;
        println!("Profile '{}' deleted successfully.", keyword);
        
        // Try to delete associated desktop shortcut
        if let Some(user_dirs) = UserDirs::new() {
            if let Some(desktop_dir) = user_dirs.desktop_dir() {
                #[cfg(target_os = "windows")]
                let shortcut_path = desktop_dir.join(format!("{}.bat", keyword));
                
                #[cfg(target_os = "linux")]
                let shortcut_path = desktop_dir.join(format!("{}.desktop", keyword));
                
                if shortcut_path.exists() {
                    match std::fs::remove_file(&shortcut_path) {
                        Ok(_) => println!(
                            "Associated desktop shortcut deleted: {}",
                            shortcut_path.display()
                        ),
                        Err(e) => eprintln!("Warning: Could not delete shortcut: {}", e),
                    }
                }
            }
        }
    } else {
        println!("Profile '{}' not found.", keyword);
    }
    Ok(())
}

fn list_profiles(profiles: &Profiles) {
    if profiles.is_empty() {
        println!("No saved profiles.");
        return;
    }
    
    println!("Saved profiles:\n");
    
    for (name, profile) in profiles {
        // Skip temp profiles
        if name.starts_with(TEMP_PROFILE_PREFIX) {
            continue;
        }
        
        println!("Profile: {}", name);
        println!("  Path: {}", profile.path.display());
        println!("  CPUs: {:?}", profile.cpus);
        
        let priority_str = profile.priority.as_ref()
            .map(|p| p.display_name())
            .unwrap_or("Normal");
        
        #[cfg(target_os = "windows")]
        let admin_note = if profile.priority.as_ref()
            .map(|p| p.requires_elevation())
            .unwrap_or(false)
        {
            " [requires admin]"
        } else {
            ""
        };
        
        #[cfg(not(target_os = "windows"))]
        let admin_note = "";
        
        println!("  Priority: {}{}", priority_str, admin_note);
        
        if let Some(attempts) = profile.retry_attempts {
            println!("  Retry attempts: {}", attempts);
        }
        
        // Validate path exists
        if !profile.path.exists() {
            println!("  WARNING: Executable not found!");
        }
        
        println!();
    }
}

fn create_shortcut(profiles: &Profiles, keyword: &str) -> Result<()> {
    let profile = profiles.get(keyword)
        .context(format!("Profile '{}' not found", keyword))?;

    let current_exe = std::env::current_exe()
        .context("Failed to get current executable path")?;
    let current_exe_str = current_exe.to_str()
        .context("Executable path contains invalid UTF-8")?;

    let user_dirs = UserDirs::new()
        .context("Could not find user directories")?;
    let desktop_dir = user_dirs.desktop_dir()
        .context("Could not find Desktop directory")?;

    #[cfg(target_os = "windows")]
    {
        let bat_path = desktop_dir.join(format!("{}.bat", keyword));
        
        // Check if elevation is needed
        let needs_admin = profile.priority.as_ref()
            .map(|p| p.requires_elevation())
            .unwrap_or(false);
        
        let content = if needs_admin {
            // Create elevated shortcut
            format!(
                "@echo off\r\n\
                 echo Requesting administrator privileges for {}...\r\n\
                 powershell -Command \"Start-Process -FilePath '{}' -ArgumentList '{}' -Verb RunAs\"\r\n",
                keyword, current_exe_str, keyword
            )
        } else {
            format!("@echo off\r\n\"{}\" {}\r\n", current_exe_str, keyword)
        };
        
        std::fs::write(&bat_path, content)
            .context("Failed to write shortcut file")?;
        
        println!("Shortcut created: {}", bat_path.display());
        
        if needs_admin {
            println!("Note: This shortcut will request administrator privileges when launched.");
            println!("Alternatively, you can:");
            println!("  - Right-click the .bat file > Properties > Advanced > Run as administrator");
            println!("  - Create a scheduled task to run without UAC prompts");
        }
    }

    #[cfg(target_os = "linux")]
    {
        let shortcut_path = desktop_dir.join(format!("{}.desktop", keyword));
        let content = format!(
            "[Desktop Entry]\n\
             Version=1.0\n\
             Name={}\n\
             Comment=Launch {} with CPU affinity and priority settings\n\
             Exec=\"{}\" {}\n\
             Terminal=false\n\
             Type=Application\n\
             Categories=Utility;\n",
            keyword,
            profile.path.display(),
            current_exe_str,
            keyword
        );
        
        std::fs::write(&shortcut_path, &content)
            .context("Failed to write .desktop file")?;
        
        let mut perms = std::fs::metadata(&shortcut_path)?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&shortcut_path, perms)
            .context("Failed to set executable permissions")?;
        
        println!("Shortcut created: {}", shortcut_path.display());
    }

    Ok(())
}

fn show_help() {
    println!();
    println!("========== affinity-rs v3 ==========");
    println!("CPU affinity and process priority launcher with profile support.\n");
    println!("USAGE:");
    println!("  affinity-rs <command>");
    println!("  affinity-rs <profile_name> [program_args...]\n");
    println!("COMMANDS:");
    println!("  list                 List all saved profiles");
    println!("  delete <profile>     Delete a saved profile and its shortcut");
    println!("  shortcut <profile>   Create a desktop shortcut for a profile");
    println!("  help                 Show this help message\n");
    println!("EXAMPLES:");
    println!("  affinity-rs list");
    println!("  affinity-rs my_game");
    println!("  affinity-rs my_game --windowed");
    println!("  affinity-rs delete my_game");
    println!("  affinity-rs shortcut my_game\n");
    println!("CREATING PROFILES:");
    println!("  Run 'affinity-rs <new_name>' to create a new profile interactively.");
    println!("  You'll be prompted for:");
    println!("    - Executable path");
    println!("    - CPU cores to use");
    println!("    - Process priority level\n");
    println!("TIPS:");
    println!("  - Add affinity-rs to your PATH to use it from anywhere");
    println!("  - High/Realtime priorities require administrator privileges on Windows");
    println!("  - For games with launchers, try targeting the game .exe directly");
    println!("  - Profiles are stored in your OS config directory\n");
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    // Load profiles with error handling
    let mut profiles = match load_profiles() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("Warning: Failed to load profiles: {}", e);
            eprintln!("Starting with empty profile list.\n");
            Profiles::new()
        }
    };
    
    // Clean up any orphaned temp profiles on startup
    let temp_keys: Vec<String> = profiles.keys()
        .filter(|k| k.starts_with(TEMP_PROFILE_PREFIX))
        .cloned()
        .collect();
    
    if !temp_keys.is_empty() {
        for key in temp_keys {
            profiles.remove(&key);
        }
        let _ = save_profiles(&profiles);
    }

    // Check for cleanup flag (used after elevation)
    let should_cleanup = args.iter().any(|arg| arg == ELEVATION_CLEANUP_FLAG);
    let args: Vec<String> = args.into_iter()
        .filter(|arg| arg != ELEVATION_CLEANUP_FLAG)
        .collect();

    if args.len() < 2 {
        show_help();
        return;
    }

    match args[1].as_str() {
        "help" | "--help" | "-h" => {
            show_help();
        }
        "list" => {
            list_profiles(&profiles);
        }
        "delete" => {
            if args.len() < 3 {
                eprintln!("Usage: affinity-rs delete <profile>");
                eprintln!("Run 'affinity-rs list' to see available profiles.");
                return;
            }
            
            match delete_profile(&mut profiles, &args[2]) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Error deleting profile: {:#}", e);
                    pause_before_exit();
                }
            }
        }
        "shortcut" => {
            if args.len() < 3 {
                eprintln!("Usage: affinity-rs shortcut <profile>");
                eprintln!("Run 'affinity-rs list' to see available profiles.");
                return;
            }
            
            match create_shortcut(&profiles, &args[2]) {
                Ok(_) => {},
                Err(e) => {
                    eprintln!("Error creating shortcut: {:#}", e);
                    pause_before_exit();
                }
            }
        }
        program_name => {
            let program_args = if args.len() > 2 { &args[2..] } else { &[] };

            if let Some(profile) = profiles.get(program_name).cloned() {
                println!("Loaded profile: '{}'", program_name);
                launch_or_exit(&profile, program_args, Some(program_name), should_cleanup);
            } else {
                // Create new profile interactively
                println!("No profile found for '{}'. Let's create one!\n", program_name);

                let path_input = match read_line("Enter full program path: ") {
                    Ok(input) => input.trim_matches('"').to_string(),
                    Err(e) => {
                        eprintln!("Error reading input: {}", e);
                        pause_before_exit();
                        return;
                    }
                };

                if path_input.is_empty() {
                    eprintln!("Error: Path cannot be empty.");
                    pause_before_exit();
                    return;
                }

                let path = PathBuf::from(&path_input);
                if !path.exists() {
                    eprintln!("Error: File not found: {}", path.display());
                    eprintln!("Please check the path and try again.");
                    pause_before_exit();
                    return;
                }

                let cpus = match get_cpu_input() {
                    Ok(cpus) => cpus,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        pause_before_exit();
                        return;
                    }
                };

                let priority = match get_priority_input() {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Error: {}", e);
                        pause_before_exit();
                        return;
                    }
                };

                let new_profile = Profile {
                    path,
                    cpus,
                    priority,
                    retry_attempts: None, // Use default
                };

                let save_choice = match read_line("\nSave this as a profile? (y/n): ") {
                    Ok(choice) => choice,
                    Err(e) => {
                        eprintln!("Error reading input: {}", e);
                        pause_before_exit();
                        return;
                    }
                };

                if save_choice.eq_ignore_ascii_case("y") {
                    let mut keyword = program_name.to_string();
                    
                    if keyword.is_empty() || profiles.contains_key(&keyword) {
                        match read_line("Enter a name for this profile: ") {
                            Ok(input) => keyword = input,
                            Err(e) => {
                                eprintln!("Error reading input: {}", e);
                                pause_before_exit();
                                return;
                            }
                        }
                    }

                    if keyword.is_empty() {
                        eprintln!("Error: Profile name cannot be empty.");
                        pause_before_exit();
                        return;
                    }

                    profiles.insert(keyword.clone(), new_profile.clone());

                    match save_profiles(&profiles) {
                        Ok(_) => println!("\nProfile '{}' saved successfully!", keyword),
                        Err(e) => {
                            eprintln!("Error saving profile: {:#}", e);
                            eprintln!("Continuing with launch anyway...");
                        }
                    }

                    launch_or_exit(&new_profile, program_args, Some(&keyword), false);
                } else {
                    println!("\nLaunching without saving profile...");
                    launch_or_exit(&new_profile, program_args, None, false);
                }
            }
        }
    }
}