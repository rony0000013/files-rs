param(
    [switch]$Build,
    [switch]$PinTaskbar
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\Files-Rust"
$StartMenuDir = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"
$ShortcutPath = Join-Path $StartMenuDir "Files.lnk"
$LogFile = Join-Path $InstallDir "auto-update.log"

New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
New-Item -ItemType Directory -Force -Path $StartMenuDir | Out-Null

function Write-Log {
    param([string]$Message)
    $ts = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $line = "[$ts] $Message"
    Write-Host $line
    Add-Content -Path $LogFile -Value $line -ErrorAction SilentlyContinue
}

function Copy-StageFile {
    param(
        [string]$Source,
        [string]$Dest
    )
    if (-not (Test-Path $Source)) {
        throw "Missing source artifact: $Source"
    }
    try {
        Copy-Item -Path $Source -Destination $Dest -Force -ErrorAction Stop
    } catch {
        # Destination is likely in use by a running Files instance: rename the running image aside
        # (Windows allows renaming a mapped executable/DLL) and place the new binary at $Dest.
        $oldPath = "$Dest.old.$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
        Move-Item -Path $Dest -Destination $oldPath -Force -ErrorAction Stop
        Copy-Item -Path $Source -Destination $Dest -Force -ErrorAction Stop
        Write-Log "Replaced in-use file (previous moved to $(Split-Path $oldPath -Leaf))"
    }
}

$createdNew = $false
$mutex = [System.Threading.Mutex]::new($false, "Local\FilesRsAutoUpdate", [ref]$createdNew)
$acquired = $false

try {
    $acquired = $mutex.WaitOne([TimeSpan]::FromMinutes(15))
    if (-not $acquired) {
        Write-Log "Timed out waiting for concurrent update lock."
        exit 1
    }

    # Best-effort cleanup of old side-renamed binaries from prior in-use updates
    Get-ChildItem -Path $InstallDir -Filter "*.old.*" -ErrorAction SilentlyContinue | ForEach-Object {
        Remove-Item -Path $_.FullName -Force -ErrorAction SilentlyContinue
    }

    $DistTargetDir = Join-Path $RepoRoot "target\app-dist"

    if ($Build) {
        Write-Log "Building release files-gpui in $DistTargetDir..."
        Push-Location $RepoRoot
        $prevEap = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        try {
            $cargoOut = & cargo build --release --bin files-gpui --target-dir $DistTargetDir 2>&1
            $exitCode = $LASTEXITCODE
            if ($exitCode -ne 0) {
                Write-Log "cargo build failed with exit code $exitCode"
                $cargoOut | ForEach-Object { Add-Content -Path $LogFile -Value "$_" -ErrorAction SilentlyContinue }
                exit $exitCode
            }
        } finally {
            $ErrorActionPreference = $prevEap
            Pop-Location
        }
    }

    $ReleaseDir = Join-Path $DistTargetDir "release"
    $ExeSrc = Join-Path $ReleaseDir "files-gpui.exe"
    $DllSrc = Join-Path $ReleaseDir "pdfium.dll"
    $IcoSrc = Join-Path $RepoRoot "assets\app_icon.ico"

    $ExeDest = Join-Path $InstallDir "files-gpui.exe"
    $DllDest = Join-Path $InstallDir "pdfium.dll"
    $IcoDest = Join-Path $InstallDir "app_icon.ico"

    Copy-StageFile -Source $ExeSrc -Dest $ExeDest
    if (Test-Path $DllSrc) {
        Copy-StageFile -Source $DllSrc -Dest $DllDest
    }
    if (Test-Path $IcoSrc) {
        Copy-Item -Path $IcoSrc -Destination $IcoDest -Force
    }

    # Create / refresh Start Menu shortcut so Windows treats it like a normal installed app
    $wsh = New-Object -ComObject WScript.Shell
    $sc = $wsh.CreateShortcut($ShortcutPath)
    $sc.TargetPath = $ExeDest
    $sc.WorkingDirectory = $env:USERPROFILE
    $sc.IconLocation = "$ExeDest,0"
    $sc.Description = "Files (files-rs)"
    $sc.Save()

    # Clear Windows icon cache entry notification so Explorer / Taskbar pick up updated icon
    try {
        Add-Type -Namespace Win32 -Name ShellNotify -MemberDefinition @'
[System.Runtime.InteropServices.DllImport("shell32.dll")]
public static extern void SHChangeNotify(int wEventId, int uFlags, System.IntPtr dwItem1, System.IntPtr dwItem2);
'@ -ErrorAction SilentlyContinue
        [Win32.ShellNotify]::SHChangeNotify(0x08000000, 0x0000, [IntPtr]::Zero, [IntPtr]::Zero)
    } catch {}

    Write-Log "Updated installed app at $ExeDest and refreshed shortcut at $ShortcutPath"

    if ($PinTaskbar) {
        $pinned = $false
        try {
            $shell = New-Object -ComObject Shell.Application
            $folder = $shell.Namespace($StartMenuDir)
            $item = $folder.ParseName("Files.lnk")
            if ($null -ne $item) {
                foreach ($verb in $item.Verbs()) {
                    $clean = ($verb.Name -replace '&', '').Trim()
                    if ($clean -match 'Pin to taskbar' -or $clean -match 'taskbarpin') {
                        $verb.DoIt()
                        $pinned = $true
                        Write-Log "Invoked Shell verb '$clean' on $ShortcutPath"
                        break
                    }
                }
            }
        } catch {
            Write-Log "Taskbar Shell verb check failed: $($_.Exception.Message)"
        }
        if (-not $pinned) {
            Write-Log "Windows 11 restricted programmatic taskbar verb; shortcut ready at $ShortcutPath"
        }
    }
} finally {
    if ($acquired) {
        $mutex.ReleaseMutex()
    }
    $mutex.Dispose()
}
