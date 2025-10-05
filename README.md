<div align="center">

# affinity-rs

A simple, cross-platform CPU affinity launcher with profile support. Pin any program to specific CPU cores and save your configurations for quick reuse.

</div>

---

## Features

- **CPU Affinity Control** - Launch programs restricted to specific CPU cores
- **Profile Management** - Save and reuse CPU affinity configurations
- **Desktop Shortcuts** - Create shortcuts to launch profiles with one click
- **Cross-Platform** - Works on both Windows and Linux
- **Fast & Lightweight** - Built with Rust for minimal overhead
- **Easy to Use** - Simple command-line interface

## Installation

### From Source

```bash
git clone https://github.com/yourusername/affinity-rs
cd affinity-rs
cargo build --release
```

The compiled binary will be in `target/release/affinity-rs` (or `affinity-rs.exe` on Windows).

### Add to PATH (Optional)

For global access, add the binary to your system PATH:

**Windows:**
```powershell
# Add the directory containing affinity-rs.exe to your PATH (current session)
$env:Path += ";C:\path\to\affinity-rs"

# For permanent access (user-level PATH)
[System.Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\path\to\affinity-rs", "User")
```

**Linux:**
```bash
# Copy to a directory in your PATH
sudo cp target/release/affinity-rs /usr/local/bin/
```

## Usage

### Quick Start

Run affinity-rs without arguments to see the help menu:

```bash
affinity-rs
```

### Commands

#### List Saved Profiles
```bash
affinity-rs list
```

#### Create and Run a New Profile
```bash
affinity-rs myapp
```

This will prompt you to:
1. Enter the full program path
2. Specify CPU cores (e.g., `0,2,4,6`)
3. Choose whether to save as a profile

#### Run a Saved Profile
```bash
affinity-rs myapp
```

If a profile named "myapp" exists, it will launch immediately with the saved settings.

#### Run a Profile with Arguments
```bash
affinity-rs myapp --arg1 value1 --arg2
```

#### Create a Desktop Shortcut
```bash
affinity-rs shortcut myapp
```

Creates a clickable shortcut on your desktop that launches the profile. On Windows, this creates a `.bat` file. On Linux, this creates a `.desktop` file.

#### Delete a Profile
```bash
affinity-rs delete myapp
```

This will delete the profile and automatically remove any associated desktop shortcut if one exists.

## Examples

### Example 1: Pin a Game to Performance Cores

```bash
# Create a profile for Far Cry 3
affinity-rs fc3

# Enter path: D:\Games\Far Cry 3\bin\farcry3_d3d11.exe
# Enter cores: 2,4,6,8
# Save profile? y

# Next time, just run:
affinity-rs fc3

# Or create a desktop shortcut:
affinity-rs shortcut fc3
```

### Example 2: Run a Video Encoder on Specific Cores

```bash
# Create a profile for HandBrake
affinity-rs handbrake

# Enter path: C:\Program Files\HandBrake\HandBrakeCLI.exe
# Enter cores: 0,1,2,3,4,5,6,7
# Save profile? y

# Run with arguments:
affinity-rs handbrake -i input.mp4 -o output.mp4
```

### Example 3: One-Time Launch Without Saving

```bash
affinity-rs temp

# Enter path: C:\Program Files\MyApp\app.exe
# Enter cores: 0,1
# Save profile? n
```

## Profile Storage

Profiles are saved in a platform-specific configuration directory:

**Windows:** `C:\Users\<YourUsername>\AppData\Roaming\affinity\AffinityRs\config\profiles.json`

**Linux:** `~/.config/affinity-rs/AffinityRs/profiles.json`

The file format is:

```json
{
  "fc3": {
    "path": "D:\\Games\\Far Cry 3\\bin\\farcry3_d3d11.exe",
    "cpus": [2, 4, 6, 8]
  },
  "handbrake": {
    "path": "C:\\Program Files\\HandBrake\\HandBrakeCLI.exe",
    "cpus": [0, 1, 2, 3, 4, 5, 6, 7]
  }
}
```

You can manually edit this file if needed.

## Use Cases

- **Old Games**: Many older games weren't designed for modern multi-core CPUs and can have compatibility issues, stuttering, or crashes when running on all cores. Limiting them to fewer cores (e.g., cores 0-3) can fix these issues
- **Legacy Applications**: Programs built for single-core or dual-core systems may perform better when restricted to specific cores
- **Hybrid CPUs**: Modern Intel and AMD processors have performance cores (P-cores) and efficiency cores (E-cores). Pin games to P-cores for better performance
- **Gaming**: Reduce stuttering and improve frame times by dedicating specific cores to your game
- **Video Encoding**: Dedicate specific cores to encoding tasks while leaving others free for system responsiveness
- **Streaming**: Separate game and streaming software (OBS, etc.) on different cores to prevent performance drops
- **Server Applications**: Isolate critical services on specific cores for predictable performance
- **Development**: Control resource usage during compilation or testing
- **Benchmarking**: Ensure consistent CPU allocation for reproducible results
- **Troubleshooting**: Test if CPU-related issues are caused by specific cores or thread scheduling

## Why Use affinity-rs Instead of Other Methods?

| Feature | Windows Task Manager | PowerShell | affinity-rs |
|---------|---------------------|------------|-------------|
| **CPU Selection** | Click checkboxes manually | Hex mask: `0x155` | Simple list: `2,4,6,8` |
| **Profiles** | None - set every time | None - write script each time | Save and reuse profiles |
| **Desktop Shortcuts** | None | Manual script creation | One command |
| **Persistence** | Resets on restart | Works at launch only | Launches with affinity every time |
| **Ease of Use** | Launch first, then set | Complex syntax | One simple command |
| **Launch Method** | Program must be running | `Start-Process -AffinityMask` | `affinity-rs mygame` |
| **Hex Calculation** | Not needed | Must calculate hex manually | Automatic conversion |
| **CLI Usage** | Cannot automate | Can script but complex | Easy scripting |
| **Cross-platform** | Windows only | Windows only | Windows and Linux |

### Example Comparison

**Windows Task Manager Method:**
1. Launch your game
2. Open Task Manager (Ctrl+Shift+Esc)
3. Find your game process
4. Right-click → Set Affinity
5. Manually click CPU 2, CPU 4, CPU 6, CPU 8 checkboxes
6. Repeat every time you launch the game

**PowerShell Method:**
```powershell
Start-Process "C:\Games\mygame.exe" -AffinityMask 0x155
```
(You need to calculate that 0x155 = CPUs 0,2,4,6,8 in hex)

**affinity-rs Method:**
```bash
affinity-rs mygame
```
Done. The game launches with the correct affinity automatically.

## Platform-Specific Notes

### Windows
- Uses the Windows API (`SetProcessAffinityMask`) to set CPU affinity
- Supports CPU cores 0-31 (32-core limit due to Windows API constraints)
- Process launches independently after affinity is set
- Desktop shortcuts are `.bat` files that call affinity-rs

### Linux
- Uses `taskset` command (must be installed)
- Install with: `sudo apt install util-linux` (usually pre-installed)
- Supports all available CPU cores
- Process launches independently after taskset applies affinity
- Desktop shortcuts are `.desktop` files with executable permissions

## Troubleshooting

### "Failed to set CPU affinity" on Windows
- Run as Administrator
- Ensure CPU numbers are valid for your system (0-indexed)
- Check that the process hasn't already exited

### "taskset: command not found" on Linux
Install util-linux:
```bash
sudo apt install util-linux
```

### Program path with spaces
The tool automatically handles paths with spaces and strips quotes if you paste them. Both formats work:
- `C:\Program Files\My Game\game.exe`
- `"C:\Program Files\My Game\game.exe"`

### Desktop shortcut not working
- **Windows**: Ensure affinity-rs.exe is in your PATH or use the full path in the .bat file
- **Linux**: Ensure the .desktop file has executable permissions (automatically set by affinity-rs)

## Building from Source

### Requirements
- Rust 1.70 or later (edition 2021)
- Cargo

### Dependencies
```toml
[dependencies]
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"
directories = "6.0.0"
anyhow = "1.0.100"
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.61.1", features = [
    "Win32_Foundation",
    "Win32_System_Threading",
] }
```

### Build
```bash
cargo build --release
```

The optimized binary will be in `target/release/`.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.

## License

MIT License - feel free to use this in your projects!

## Credits

Built with Rust using:
- `windows-sys` - Official Microsoft Windows API bindings
- `directories` - Cross-platform configuration paths
- `serde` & `serde_json` - Serialization
- `anyhow` - Error handling

CPU affinity is controlled via Microsoft's official windows-sys API on Windows and `taskset` on Linux.

---

**Note**: CPU affinity settings affect how the operating system schedules threads. Use responsibly and understand your system's CPU topology before pinning critical applications.
