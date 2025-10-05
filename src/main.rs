use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::{Command, exit};

use directories::{ProjectDirs, UserDirs};
#[cfg(target_os = "linux")]
use std::os::unix::fs::PermissionsExt;

const PROFILE_FILE_NAME: &str = "profiles.json";

#[derive(Serialize, Deserialize, Debug, Clone)]
struct Profile {
    path: PathBuf,
    cpus: Vec<usize>,
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

fn load_profiles() -> Profiles {
    let profile_path = match get_profile_path() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("Warning: Could not determine profile file path: {}", e);
            return Profiles::new();
        }
    };

    match std::fs::read_to_string(profile_path) {
        Ok(data) => serde_json::from_str(&data).unwrap_or_else(|_| Profiles::new()),
        Err(_) => Profiles::new(),
    }
}

fn save_profiles(profiles: &Profiles) -> Result<()> {
    let profile_path = get_profile_path()?;
    let data = serde_json::to_string_pretty(profiles).context("Failed to serialize profiles")?;
    std::fs::write(profile_path, data).context("Failed to write profiles to disk")?;
    Ok(())
}

fn pause_before_exit() {
    print!("\nPress Enter to exit...");
    io::stdout().flush().unwrap_or_default();
    let mut dummy = String::new();
    io::stdin().read_line(&mut dummy).unwrap_or_default();
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

fn launch_profile(profile: &Profile, args: &[String]) -> Result<()> {
    println!(
        "\nLaunching '{}' with CPU affinity: {:?}",
        profile.path.display(),
        profile.cpus
    );
    if !args.is_empty() {
        println!("Arguments: {:?}", args);
    }

    #[cfg(target_os = "linux")]
    {
        let cpu_str = profile
            .cpus
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let mut cmd = Command::new("taskset");
        cmd.arg("-c").arg(&cpu_str).arg(&profile.path).args(args);
        let child = cmd
            .spawn()
            .context("Failed to spawn process with taskset")?;
        println!("Process launched! PID: {}", child.id());
        println!("\nProgram is running independently.");
    }

    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{
            GetProcessAffinityMask, OpenProcess, PROCESS_QUERY_INFORMATION,
            PROCESS_SET_INFORMATION, SetProcessAffinityMask,
        };

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
            bail!("No valid CPUs specified");
        }

        let child = Command::new(&profile.path)
            .args(args)
            .spawn()
            .context("Failed to spawn process")?;
        let pid = child.id();
        println!("Process launched with PID: {}", pid);

        std::thread::sleep(std::time::Duration::from_millis(100));

        unsafe {
            let handle = OpenProcess(PROCESS_SET_INFORMATION | PROCESS_QUERY_INFORMATION, 0, pid);
            if handle.is_null() {
                bail!("Failed to open process handle");
            }

            let result = SetProcessAffinityMask(handle, affinity_mask);
            if result == 0 {
                eprintln!("Failed to set CPU affinity");
            } else {
                println!("✓ CPU affinity set successfully!");
            }

            let mut process_affinity: usize = 0;
            let mut system_affinity: usize = 0;
            GetProcessAffinityMask(handle, &mut process_affinity, &mut system_affinity);
            println!(
                "Requested mask: 0x{:X}, Actual mask: 0x{:X}",
                affinity_mask, process_affinity
            );

            CloseHandle(handle);
        }

        println!("\nProgram is running independently.");
    }

    Ok(())
}

fn launch_or_exit(profile: &Profile, args: &[String]) -> ! {
    match launch_profile(profile, args) {
        Ok(_) => exit(0),
        Err(e) => {
            eprintln!("Error launching program: {:#}", e);
            pause_before_exit();
            exit(1);
        }
    }
}

fn delete_profile(profiles: &mut Profiles, keyword: &str) -> Result<()> {
    if profiles.remove(keyword).is_some() {
        save_profiles(profiles)?;
        println!("✓ Profile '{}' deleted!", keyword);
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
    println!("Saved profiles:");
    for (k, p) in profiles {
        println!("- {} => {:?}, path: {}", k, p.cpus, p.path.display());
    }
}

fn create_shortcut(profiles: &Profiles, keyword: &str) -> Result<()> {
    let current_exe = std::env::current_exe().context("Failed to get current executable path")?;
    let current_exe_str = current_exe
        .to_str()
        .context("Executable path contains invalid UTF-8")?;

    let user_dirs = UserDirs::new().context("Could not find user directories")?;
    let desktop_dir = user_dirs
        .desktop_dir()
        .context("Could not find Desktop directory")?;

    #[cfg(target_os = "windows")]
    {
        profiles
            .get(keyword)
            .context(format!("Profile '{}' not found", keyword))?;

        let bat_path = desktop_dir.join(format!("{}.bat", keyword));
        let content = format!("@echo off\r\n\"{}\" {}\r\n", current_exe_str, keyword);
        std::fs::write(&bat_path, content).context("Failed to write shortcut file")?;
        println!("✓ Shortcut created on your Desktop: {}", bat_path.display());
    }

    #[cfg(target_os = "linux")]
    {
        let profile = profiles
            .get(keyword)
            .context(format!("Profile '{}' not found", keyword))?;

        let shortcut_path = desktop_dir.join(format!("{}.desktop", keyword));
        let content = format!(
            "[Desktop Entry]\n\
             Version=1.0\n\
             Name={}\n\
             Comment=Launch {} with CPU affinity\n\
             Exec=\"{}\" {}\n\
             Terminal=false\n\
             Type=Application\n",
            keyword,
            profile.path.display(),
            current_exe_str,
            keyword
        );
        std::fs::write(&shortcut_path, &content).context("Failed to write .desktop file")?;
        let mut perms = std::fs::metadata(&shortcut_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&shortcut_path, perms)
            .context("Failed to set executable permissions")?;
        println!(
            "✓ Shortcut created on your Desktop: {}",
            shortcut_path.display()
        );
    }

    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut profiles = load_profiles();

    if args.len() < 2 {
        println!();
        println!("========== affinity-rs ==========");
        println!("A simple CPU affinity launcher with profile support.\n");
        println!("Usage:");
        println!("  affinity-rs <command>");
        println!("  affinity-rs <profile | program> [program_args...]\n");
        println!("Commands:");
        println!("  list                 List all saved profiles");
        println!("  delete <profile>     Delete a saved profile");
        println!("  shortcut <profile>   Create a desktop shortcut for a profile\n");
        println!("Examples:");
        println!("  affinity-rs list");
        println!("  affinity-rs fc3");
        println!("  affinity-rs delete fc3");
        println!("  affinity-rs shortcut fc3");
        println!("  affinity-rs firefox\n");
        println!("Tip: Add affinity-rs to your system PATH to run it globally from anywhere.\n");
        return;
    }

    match args[1].as_str() {
        "list" => {
            list_profiles(&profiles);
        }
        "delete" => {
            if args.len() < 3 {
                eprintln!("Usage: affinity-rs delete <profile>");
                return;
            }
            if let Err(e) = delete_profile(&mut profiles, &args[2]) {
                eprintln!("\nError: Failed to delete profile: {:#}", e);
                pause_before_exit();
            }
        }
        "shortcut" => {
            if args.len() < 3 {
                eprintln!("Usage: affinity-rs shortcut <profile>");
                return;
            }
            if let Err(e) = create_shortcut(&profiles, &args[2]) {
                eprintln!("Error creating shortcut: {:#}", e);
                pause_before_exit();
            }
        }
        program_name => {
            let program_args = if args.len() > 2 { &args[2..] } else { &[] };

            if let Some(profile) = profiles.get(program_name) {
                println!("✓ Loaded saved profile '{}'", program_name);
                launch_or_exit(profile, program_args);
            } else {
                println!(
                    "No saved profile for '{}'. Creating new profile.",
                    program_name
                );

                let path_input = read_line("Enter full program path: ").unwrap_or_default();
                let path_input = path_input.trim_matches('"');
                let cpus = get_cpu_input().unwrap_or_default();

                if path_input.is_empty() || cpus.is_empty() {
                    eprintln!("\nInvalid path or CPU core selection. Aborting profile creation.");
                    pause_before_exit();
                    return;
                }

                let save_choice = read_line("Save this as a profile? (y/n): ").unwrap_or_default();
                if save_choice.to_lowercase() == "y" {
                    let mut keyword = program_name.to_string();
                    if keyword.is_empty() {
                        keyword = read_line("Enter keyword for profile: ").unwrap_or_default();
                    }

                    let new_profile = Profile {
                        path: PathBuf::from(path_input),
                        cpus: cpus.clone(),
                    };
                    profiles.insert(keyword, new_profile.clone());

                    if let Err(e) = save_profiles(&profiles) {
                        eprintln!("\nError: Failed to save profile: {:#}", e);
                        pause_before_exit();
                    } else {
                        println!("✓ Profile saved!");
                    }

                    launch_or_exit(&new_profile, program_args);
                } else {
                    println!("Launching without saving profile...");
                    let temp_profile = Profile {
                        path: PathBuf::from(path_input),
                        cpus,
                    };
                    launch_or_exit(&temp_profile, program_args);
                }
            }
        }
    }
}
