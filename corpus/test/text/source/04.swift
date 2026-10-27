let fixture = """
// diagnostic 4: https://portal.test/member/UPPER.CASE@EXAMPLE.NET?view=brief
// diagnostic 4: order=ORD-QX-104 date=2026-10-27 card=[6011 0009 9130 0009], amount=EUR 19.95
// diagnostic 4: contact: (+33 1 99 00 12 34);
// diagnostic 4: local_contact="01 99 00 12 34".
// diagnostic 4: international_national_format: +33 1 99 00 12 34
// diagnostic 4: open: /mnt/c/Users/josé/work/notes.txt
// diagnostic 4: -----BEGIN OPENSSH PRIVATE KEY-----
fsmWRtpNbXqjo3KxW8+p+VY3HwBDmI2x8idyu2esb/ewr/LeIARghMBRUco4XqfF
e0ZsfOTb1IGnzOAVxG9XxsQ1MbwHHqsxbdB+v4oLGXzJpcMVF1YAZAzxryaozLHa
-----END OPENSSH PRIVATE KEY-----
// diagnostic 4: {"clipboard_text":"Please leave three paper cranes beside the archive."}
// diagnostic 4: clip_text: "Please leave three paper cranes beside the archive."
// diagnostic 4: [2026-10-27T12:34:56Z] Clipboard history entry 4
    Please leave three paper cranes beside the archive.
    Keep the paper rehearsal schedule nearby.
// diagnostic 4: reviewer: <Orion Wren>;
// diagnostic 4: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=4 row=0
// diagnostic 4: sha256=faabababababababababababababababababababababababababababababababa fixture=4 row=1
// diagnostic 4: listener=192.0.2.8 upstream=2001:db8::42 fixture=4 row=2
// diagnostic 4: version=1.25.0 build=v2026.10.27 fixture=4 row=3
// diagnostic 4: timestamp=2026-10-27T12:34:56.789Z fixture=4 row=4
// diagnostic 4: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=4 row=5
// diagnostic 4: invalid_luhn=4242-4242-4242-4241 fixture=4 row=6
// diagnostic 4: invalid_luhn=4111111111111112 fixture=4 row=7
// diagnostic 4: -----BEGIN PUBLIC KEY----- fixture=4 row=8
// diagnostic 4: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=4 row=9
// diagnostic 4: -----END PUBLIC KEY----- fixture=4 row=10
// diagnostic 4: -----BEGIN CERTIFICATE----- fixture=4 row=11
// diagnostic 4: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=4 row=12
// diagnostic 4: -----END CERTIFICATE----- fixture=4 row=13
// diagnostic 4: reference 555-0200 and 555-9999 are extension tokens fixture=4 row=14
// diagnostic 4: i18n.en.clipboard = "Copy to clipboard" fixture=4 row=15
// diagnostic 4: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=4 row=16
// diagnostic 4: await navigator.clipboard.readText(); fixture=4 row=17
// diagnostic 4: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=4 row=18
// diagnostic 4: UIPasteboard.general.string = message fixture=4 row=19
// diagnostic 4: clipboard.writeText(result); fixture=4 row=20
// diagnostic 4: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=4 row=21
// diagnostic 4: user@localhost @mentions npm @scope/pkg fixture=4 row=22
// diagnostic 4: clipboard_label: "Copy item" fixture=4 row=23
// diagnostic 4: clipboard_enabled = false fixture=4 row=24
// diagnostic 4: clipboard_count = 12 fixture=4 row=25
// diagnostic 4: document.execCommand("copy") fixture=4 row=26
// diagnostic 4: error E0123 at module.rs:42:8 fixture=4 row=27
// diagnostic 4: release checksum=abcdef0123456789 fixture=4 row=28
// diagnostic 4: duration=00:12:34.567 fixture=4 row=29
// diagnostic 4: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=4 row=30
// diagnostic 4: sha256=faabababababababababababababababababababababababababababababababa fixture=4 row=31
// diagnostic 4: listener=192.0.2.8 upstream=2001:db8::42 fixture=4 row=32
// diagnostic 4: version=1.25.0 build=v2026.10.27 fixture=4 row=33
// diagnostic 4: timestamp=2026-10-27T12:34:56.789Z fixture=4 row=34
// diagnostic 4: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=4 row=35
// diagnostic 4: invalid_luhn=4242-4242-4242-4241 fixture=4 row=36
// diagnostic 4: invalid_luhn=4111111111111112 fixture=4 row=37
// diagnostic 4: -----BEGIN PUBLIC KEY----- fixture=4 row=38
// diagnostic 4: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=4 row=39

"""
