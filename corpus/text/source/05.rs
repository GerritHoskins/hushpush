const FIXTURE: &str = r###"
// diagnostic 5: https://portal.test/member/UPPER.CASE@EXAMPLE.NET?view=brief
// diagnostic 5: order=ORD-QX-105 date=2026-10-27 card=[6011 0009 9130 0009], amount=EUR 19.95
// diagnostic 5: contact: (+33 1 99 00 12 34);
// diagnostic 5: local_contact="01 99 00 12 34".
// diagnostic 5: international_national_format: +33 1 99 00 12 34
// diagnostic 5: open: /mnt/c/Users/josé/work/notes.txt
// diagnostic 5: -----BEGIN OPENSSH PRIVATE KEY-----
EekySe16yW0T6jfZOyocpd+3LbZ/8Qf+HLd0Y7gfI67/P/wyW0+Q/RStTspGgXIO
zwfsv3fGqMf4MfQ6pH7pu96xIbKRIx4xXwUTt0OoGS3Ah+fF/WKXmE7gJTMPiZRr
-----END OPENSSH PRIVATE KEY-----
// diagnostic 5: {"clipboard_text":"Please leave three paper cranes beside the archive."}
// diagnostic 5: clip_text: "Please leave three paper cranes beside the archive."
// diagnostic 5: [2026-10-27T12:34:56Z] Clipboard history entry 5
    Please leave three paper cranes beside the archive.
    Keep the paper rehearsal schedule nearby.
// diagnostic 5: reviewer: <Orion Wren>;
// diagnostic 5: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=5 row=0
// diagnostic 5: sha256=faabababababababababababababababababababababababababababababababa fixture=5 row=1
// diagnostic 5: listener=192.0.2.8 upstream=2001:db8::42 fixture=5 row=2
// diagnostic 5: version=1.25.0 build=v2026.10.27 fixture=5 row=3
// diagnostic 5: timestamp=2026-10-27T12:34:56.789Z fixture=5 row=4
// diagnostic 5: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=5 row=5
// diagnostic 5: invalid_luhn=4242-4242-4242-4241 fixture=5 row=6
// diagnostic 5: invalid_luhn=4111111111111112 fixture=5 row=7
// diagnostic 5: -----BEGIN PUBLIC KEY----- fixture=5 row=8
// diagnostic 5: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=5 row=9
// diagnostic 5: -----END PUBLIC KEY----- fixture=5 row=10
// diagnostic 5: -----BEGIN CERTIFICATE----- fixture=5 row=11
// diagnostic 5: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=5 row=12
// diagnostic 5: -----END CERTIFICATE----- fixture=5 row=13
// diagnostic 5: reference 555-0200 and 555-9999 are extension tokens fixture=5 row=14
// diagnostic 5: i18n.en.clipboard = "Copy to clipboard" fixture=5 row=15
// diagnostic 5: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=5 row=16
// diagnostic 5: await navigator.clipboard.readText(); fixture=5 row=17
// diagnostic 5: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=5 row=18
// diagnostic 5: UIPasteboard.general.string = message fixture=5 row=19
// diagnostic 5: clipboard.writeText(result); fixture=5 row=20
// diagnostic 5: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=5 row=21
// diagnostic 5: user@localhost @mentions npm @scope/pkg fixture=5 row=22
// diagnostic 5: clipboard_label: "Copy item" fixture=5 row=23
// diagnostic 5: clipboard_enabled = false fixture=5 row=24
// diagnostic 5: clipboard_count = 12 fixture=5 row=25
// diagnostic 5: document.execCommand("copy") fixture=5 row=26
// diagnostic 5: error E0123 at module.rs:42:8 fixture=5 row=27
// diagnostic 5: release checksum=abcdef0123456789 fixture=5 row=28
// diagnostic 5: duration=00:12:34.567 fixture=5 row=29
// diagnostic 5: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=5 row=30
// diagnostic 5: sha256=faabababababababababababababababababababababababababababababababa fixture=5 row=31
// diagnostic 5: listener=192.0.2.8 upstream=2001:db8::42 fixture=5 row=32
// diagnostic 5: version=1.25.0 build=v2026.10.27 fixture=5 row=33
// diagnostic 5: timestamp=2026-10-27T12:34:56.789Z fixture=5 row=34
// diagnostic 5: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=5 row=35
// diagnostic 5: invalid_luhn=4242-4242-4242-4241 fixture=5 row=36
// diagnostic 5: invalid_luhn=4111111111111112 fixture=5 row=37
// diagnostic 5: -----BEGIN PUBLIC KEY----- fixture=5 row=38
// diagnostic 5: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=5 row=39

"###;
