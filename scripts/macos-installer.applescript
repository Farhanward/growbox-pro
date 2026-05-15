-- GrowBox Pro Installer
-- Compiled via: osacompile -o "Install GrowBox Pro.app" macos-installer.applescript

set installerPath to POSIX path of (path to me)
set dmgDir to do shell script "dirname " & quoted form of installerPath
set appSource to do shell script "find " & quoted form of dmgDir & " -name '*.app' -maxdepth 1 -not -name 'Install*' | head -1"
set appName to do shell script "basename " & quoted form of appSource

-- Confirm
set userChoice to button returned of (display dialog "Install " & appName & " to your Applications folder?" ¬
    buttons {"Cancel", "Install"} ¬
    default button "Install" ¬
    with title "GrowBox Pro Installer" ¬
    with icon note)

if userChoice is "Cancel" then return

-- Install silently (no Terminal shown)
try
    do shell script "rm -rf " & quoted form of ("/Applications/" & appName)
    do shell script "cp -R " & quoted form of appSource & " /Applications/"
    do shell script "xattr -cr " & quoted form of ("/Applications/" & appName)
    do shell script "open " & quoted form of ("/Applications/" & appName)
    display dialog "GrowBox Pro has been installed and launched!" ¬
        buttons {"Done"} default button "Done" ¬
        with title "Installation Complete" ¬
        with icon note
on error errMsg
    display dialog "Installation failed." & return & return & errMsg ¬
        buttons {"OK"} default button "OK" ¬
        with title "Installation Error" ¬
        with icon stop
end try
