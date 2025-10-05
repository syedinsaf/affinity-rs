<div align="center">

# affinity-rs

A simple, cross-platform CPU affinity launcher with profile support. Pin any program to specific CPU cores and save your configurations for quick reuse.

</div>

---

## Features

- **CPU Affinity Control** - Pin programs to specific CPU cores
- **Process Priority Management** - Set priority levels (Idle to Realtime)
- **Profile System** - Save and reuse configurations
- **Desktop Shortcuts** - One-click launching with auto-elevation support
- **Smart Retry Logic** - Handles game launchers that spawn separate processes
- **Profile Validation** - Detects missing executables and invalid CPU assignments
- **Cross-Platform** - Windows and Linux support
- **Zero Overhead** - Sets affinity/priority then exits, no background process

## Installation

### From Source

```bash
git clone https://github.com/yourusername/affinity-rs
cd affinity-rs
cargo build --release
```

Binary location: `target/release/affinity-rs` (or `affinity-rs.exe` on Windows)

### Add to PATH (Recommended)

**Windows:**
```powershell
# Add to user PATH permanently
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
[Environment]::SetEnvironmentVariable("Path", "$userPath;C:\path\to\affinity-rs", "User")
```

**Linux:**
```bash
sudo cp target/release/affinity-rs /usr/local/bin/
```

## Quick Start

```bash
# Show help
affinity-rs help

# Create a new profile interactively
affinity-rs mygame

# Launch a saved profile
affinity-rs mygame

# List all profiles
affinity-rs list

# Create desktop shortcut
affinity-rs shortcut mygame

# Delete profile
affinity-rs delete mygame
```

## Usage

### Creating Profiles

Run `affinity-rs <profile_name>` for any new name:

```bash
affinity-rs fc3
```

You'll be prompted for:
1. **Executable path** - Full path to your program
2. **CPU cores** - Comma-separated list (e.g., `0,2,4,6`)
3. **Priority level** - Choose from 6 options:
   - Idle
   - Below Normal
   - Normal (default)
   - Above Normal
   - High (requires admin on Windows)
   - Realtime (requires admin on Windows - use with caution!)
4. **Save profile** - Choose `y` to save, `n` for one-time launch

### Process Priority Levels

| Priority | Use Case | Admin Required (Windows) |
|----------|----------|--------------------------|
| **Idle** | Background tasks that should never interfere | No |
| **Below Normal** | Low-priority background work | No |
| **Normal** | Standard applications (default) | No |
| **Above Normal** | Games and important applications | No |
| **High** | Critical real-time applications | Yes |
| **Realtime** | Time-critical systems only - can freeze your PC! | Yes |

**Warning**: Realtime priority can make your system unresponsive. Only use it if you understand the risks.

### Windows Elevation (High/Realtime Priority)

On Windows, High and Realtime priorities require administrator privileges. When needed:

1. affinity-rs automatically requests UAC elevation
2. A new elevated window opens and launches your program
3. The original window closes

For unsaved profiles with High/Realtime priority, a temporary profile is created, used for elevation, then automatically cleaned up.

**Desktop shortcuts for elevated profiles** automatically request admin privileges when clicked.

### Launching with Arguments

Pass arguments after the profile name:

```bash
affinity-rs mygame --fullscreen --resolution 1920x1080
```

### Desktop Shortcuts

```bash
affinity-rs shortcut mygame
```

Creates a clickable shortcut on your desktop:
- **Windows**: `.bat` file (auto-elevates if High/Realtime priority)
- **Linux**: `.desktop` file with executable permissions

### Profile Management

```bash
# List all saved profiles
affinity-rs list

# Output shows:
# - Profile name
# - Executable path
# - CPU cores assigned
# - Priority level
# - [requires admin] badge if applicable
# - Warning if executable not found

# Delete a profile and its shortcut
affinity-rs delete mygame
```

### Profile Storage

Profiles are stored in JSON format:

**Windows**: `%APPDATA%\affinity\AffinityRs\config\profiles.json`

**Linux**: `~/.config/affinity-rs/AffinityRs/profiles.json`

Example `profiles.json`:
```json
{
  "fc3": {
    "path": "D:\\Games\\Far Cry 3\\bin\\farcry3_d3d11.exe",
    "cpus": [2, 4, 6, 8],
    "priority": "above_normal"
  },
  "encoder": {
    "path": "/usr/bin/ffmpeg",
    "cpus": [0, 1, 2, 3],
    "priority": "below_normal",
    "retry_attempts": 3
  }
}
```

You can manually edit this file to:
- Change paths
- Adjust CPU assignments
- Modify priority levels
- Set custom retry attempts (default: 5)

## Use Cases

### Gaming

**Old games with multi-core issues**:
Many older games have bugs when running on modern CPUs:
```bash
affinity-rs oldgame
# Assign to CPUs: 0,1,2,3
# Priority: Above Normal
```

**Hybrid CPU optimization** (Intel 12th gen+, AMD Ryzen 7000+):
Pin games to performance cores only:
```bash
# Intel P-cores are typically 0,2,4,6,8,10...
affinity-rs mygame
# Assign to CPUs: 0,2,4,6,8,10
# Priority: High (requires admin)
```

**Reduce stuttering**:
Dedicating specific cores can improve frame times and reduce microstutter.

### Content Creation

**Video encoding**:
```bash
affinity-rs handbrake
# Assign to CPUs: 0,1,2,3,4,5,6,7
# Priority: Below Normal
# Encodes in background without affecting foreground tasks
```

**Streaming**:
Separate game and OBS on different cores:
```bash
# Game on P-cores
affinity-rs game
# Assign to CPUs: 0,2,4,6

# OBS on E-cores  
affinity-rs obs
# Assign to CPUs: 8,9,10,11
```

### Development

**Compilation**:
```bash
affinity-rs build
# Assign to CPUs: 0,1,2,3,4,5,6,7
# Priority: Below Normal
# Build in background while working
```

**Testing**:
Reproduce issues on specific core configurations.

### Servers & Services

**Database isolation**:
```bash
affinity-rs postgres
# Assign to CPUs: 0,1,2,3
# Priority: High
# Dedicated cores for predictable performance
```

## Why Use affinity-rs?

### vs. Windows Task Manager

| Task Manager | affinity-rs |
|--------------|-------------|
| Launch program first | Launch with affinity |
| Open Task Manager every time | One command |
| Click checkboxes manually | Simple list: `0,2,4` |
| No persistence | Saved profiles |
| No priority on launch | Set priority immediately |
| No automation | Script-friendly |

### vs. PowerShell

| PowerShell | affinity-rs |
|------------|-------------|
| `Start-Process -AffinityMask 0x155` | `affinity-rs game` |
| Calculate hex masks | Use decimal CPU numbers |
| No profile system | Save and reuse |
| Complex scripts | Simple commands |
| No auto-elevation | Automatic UAC prompts |

### vs. Start /AFFINITY (Command Prompt)

| CMD | affinity-rs |
|-----|-------------|
| `start /affinity 55 game.exe` | `affinity-rs game` |
| Hex mask required | Decimal list |
| No priority control | Full priority support |
| Windows only | Cross-platform |

## Platform-Specific Details

### Windows

- Uses `SetProcessAffinityMask` and `SetPriorityClass` Win32 APIs
- Retries up to 5 times (configurable) to handle launcher → game transitions
- Detects when launchers spawn separate processes
- Automatic UAC elevation for High/Realtime priorities
- Verifies affinity/priority were successfully applied

**Known limitation**: Some games with anti-cheat or launchers may reset their own priority. This is normal and not a bug in affinity-rs.

### Linux

- Uses `taskset` command (must be installed)
- Uses `nice` for priority control
- Install if missing: `sudo apt install util-linux`

**Priority mapping**:
- Idle → nice 19
- Below Normal → nice 10
- Normal → nice 0
- Above Normal → nice -5
- High → nice -10
- Realtime → nice -20

Negative nice values may require `sudo` or appropriate permissions.

## Troubleshooting

### "Failed to set CPU affinity"

**Cause**: Process exited too quickly or invalid CPU numbers

**Solutions**:
- Verify CPU numbers exist on your system (run `affinity-rs list` to see warnings)
- Try increasing retry attempts by manually editing `profiles.json`:
  ```json
  "retry_attempts": 10
  ```
- For games with launchers, target the actual game .exe directly

### Profile validation failed: Executable not found

Your executable was moved or deleted. Options:
1. Update path: Choose option 1 when prompted
2. Delete profile: `affinity-rs delete profilename`
3. Manually edit `profiles.json`

### UAC prompt appears every time (Windows)

This is normal for High/Realtime priorities. To avoid:
1. Use Normal or Above Normal priority instead
2. Right-click the .bat shortcut → Properties → Advanced → "Run as administrator"
3. Create a Windows scheduled task (advanced users)

### "taskset: command not found" (Linux)

```bash
sudo apt install util-linux
```

### Warning: CPU X references CPU beyond system count

Your profile specifies a CPU that doesn't exist on this system (e.g., CPU 15 on an 8-core system). The OS will ignore invalid cores. To fix:

```bash
# Check your CPU count
nproc  # Linux
wmic cpu get NumberOfLogicalProcessors  # Windows

# Update profile
affinity-rs delete oldprofile
affinity-rs newprofile  # Create with correct CPUs
```

### Process reset its priority

Some applications (especially games) intentionally reset their own priority after launch. This is normal. affinity-rs sets priority at launch, but can't prevent the application from changing it later.

## Building from Source

### Requirements

- Rust 1.70+ (2021 edition)
- Cargo

### Dependencies

```toml
[dependencies]
anyhow = "1.0.100"
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
directories = "6.0.0"
num_cpus = "1.16"

[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61.1", features = [
    "Win32_Foundation",
    "Win32_System_Threading",
    "Win32_Security",
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
] }
```

### Build Commands

```bash
# Development build
cargo build

# Optimized release build
cargo build --release

# Run tests
cargo test

# Check for errors without building
cargo check
```

## Advanced Usage

### Manual Profile Editing

Edit `profiles.json` directly for batch changes:

```json
{
  "game1": {
    "path": "C:\\Games\\game1.exe",
    "cpus": [0, 2, 4, 6],
    "priority": "high",
    "retry_attempts": 10
  }
}
```

Fields:
- `path` (required): Full path to executable
- `cpus` (required): Array of CPU core numbers (0-indexed)
- `priority` (optional): `idle`, `below_normal`, `normal`, `above_normal`, `high`, `realtime`
- `retry_attempts` (optional): Number of times to retry setting affinity (default: 5)

### Scripting & Automation

Launch profiles from scripts:

```bash
# Batch file (Windows)
@echo off
affinity-rs game1
affinity-rs encoder
```

```bash
# Shell script (Linux)
#!/bin/bash
affinity-rs game1 &
sleep 2
affinity-rs voice-chat &
```

### Finding CPU Core Numbers

**Windows PowerShell**:
```powershell
# Show logical processor count
(Get-WmiObject Win32_Processor).NumberOfLogicalProcessors

# View core layout
Get-WmiObject Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors
```

**Linux**:
```bash
# Show CPU count
nproc

# View detailed CPU info
lscpu

# View per-core info
cat /proc/cpuinfo | grep processor
```

For hybrid CPUs (Intel 12th gen+), P-cores typically come first. Check your BIOS or CPU-Z for exact mapping.

## Performance Tips

1. **Don't over-restrict**: Leaving at least 2 cores free helps system responsiveness
2. **Test different configurations**: Profile multiple variations and test which works best
3. **Monitor performance**: Use Task Manager (Windows) or `htop` (Linux) to verify affinity is working
4. **Launcher vs Game**: If using a game launcher, target the actual game .exe for better results
5. **Priority abuse**: Don't set everything to High/Realtime - it defeats the purpose

## Known Limitations

- Windows API limits affinity to 64 cores maximum (most systems have far fewer)
- Some protected processes (system services, anti-cheat) cannot have affinity modified
- Applications can reset their own priority after launch (by design)
- Game launchers that spawn separate processes may require manual targeting of the game .exe

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## Credits

Built with:
- [Rust](https://www.rust-lang.org/) - Systems programming language
- [windows-sys](https://github.com/microsoft/windows-rs) - Windows API bindings
- [serde](https://serde.rs/) - Serialization framework
- [directories](https://github.com/dirs-dev/directories-rs) - Platform dirs
- [anyhow](https://github.com/dtolnay/anyhow) - Error handling
- [num_cpus](https://github.com/seanmonstar/num_cpus) - CPU detection

---

## License

MIT License - feel free to use this in your projects!

---

**Disclaimer**: CPU affinity and process priority are advanced system features. Improper use (especially Realtime priority) can cause system instability. Use responsibly and understand your hardware before making changes.
