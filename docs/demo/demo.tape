# The README demo: two sessions lease `cargo build --release`, and the second one queues.
# Run it through record.sh, which adds Output and Theme and starts the sessions.

Set Shell powershell
Set FontSize 28
Set Width 1600
Set Height 640
Set Padding 36
Set Framerate 30
Set TypingSpeed 60ms
Set WindowBar Colorful
Set BorderRadius 8

# The throwaway home and the trigger file that starts the sessions' timeline.
Hide
Type "$env:USERPROFILE = $env:ALLOT_DEMO_HOME; $env:PATH = $env:ALLOT_DEMO_BIN + ';' + $env:PATH"
Enter
Type "function prompt { '$ ' }; New-Item -ItemType File $env:ALLOT_DEMO_GO | Out-Null; Clear-Host"
Enter
Sleep 1s
Show

Type "allot status --watch --interval 1"
Sleep 400ms
Enter
Sleep 15s
