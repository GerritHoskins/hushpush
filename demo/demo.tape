Output "/vhs/demo/demo.gif"
Set Shell bash
Set FontSize 22
Set Width 1280
Set Height 800
Set Framerate 10
Set TypingSpeed 0ms
Set Padding 16
Set Theme "Dracula"
Hide
Type "source /vhs/demo/setup.sh; set +e; clear"
Enter
Sleep 1s
Show
Type "hushpush scan push --include-reserved --no-user-patterns"
Enter
Sleep 7s
Hide
Type "clear"
Enter
Sleep 250ms
Show
Type "hushpush redact evidence --out reviewed --include-reserved --no-user-patterns"
Enter
Sleep 6s
Hide
Type "clear"
Enter
Sleep 250ms
Show
Type "hushpush scan dir reviewed --include-reserved --no-user-patterns"
Enter
Sleep 6s
