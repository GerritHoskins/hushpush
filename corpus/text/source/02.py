FIXTURE = r"""
// diagnostic 2: owner='robot.42@unit.example';
// diagnostic 2: order=ORD-QX-102 date=2026-10-27 card=[5105 1051 0510 5100], amount=EUR 19.95
// diagnostic 2: contact: ((555) 010-2345);
// diagnostic 2: local_contact="555-0134".
// diagnostic 2: international_national_format: +61 2 5550 1234
// diagnostic 2: open: /home/sam.dev/src/app
// diagnostic 2: private_key: |
    -----BEGIN EC PRIVATE KEY-----
    QZKIFdmd5OymyyTa0PaVl/oVKydLnwJRDhbCpriNw3F3H0M5jp8BsBYIDEYCdKqw
    a46TD4Pd8KLATNQYC9Yh9Rs7DqI4FsYWCc3JNEAnFEMxiTEwrhezZ7tIxYDtsM0O
    -----END EC PRIVATE KEY-----
// diagnostic 2: [2026-10-27 12:34:56] Clipboard history entry 2
	The rehearsal starts after the brass bell rings.
	Keep the paper rehearsal schedule nearby.
// diagnostic 2: reviewer: <Orion Wren>;
// diagnostic 2: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=2 row=0
// diagnostic 2: sha256=faabababababababababababababababababababababababababababababababa fixture=2 row=1
// diagnostic 2: listener=192.0.2.8 upstream=2001:db8::42 fixture=2 row=2
// diagnostic 2: version=1.25.0 build=v2026.10.27 fixture=2 row=3
// diagnostic 2: timestamp=2026-10-27T12:34:56.789Z fixture=2 row=4
// diagnostic 2: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=2 row=5
// diagnostic 2: invalid_luhn=4242-4242-4242-4241 fixture=2 row=6
// diagnostic 2: invalid_luhn=4111111111111112 fixture=2 row=7
// diagnostic 2: -----BEGIN PUBLIC KEY----- fixture=2 row=8
// diagnostic 2: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=2 row=9
// diagnostic 2: -----END PUBLIC KEY----- fixture=2 row=10
// diagnostic 2: -----BEGIN CERTIFICATE----- fixture=2 row=11
// diagnostic 2: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=2 row=12
// diagnostic 2: -----END CERTIFICATE----- fixture=2 row=13
// diagnostic 2: reference 555-0200 and 555-9999 are extension tokens fixture=2 row=14
// diagnostic 2: i18n.en.clipboard = "Copy to clipboard" fixture=2 row=15
// diagnostic 2: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=2 row=16
// diagnostic 2: await navigator.clipboard.readText(); fixture=2 row=17
// diagnostic 2: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=2 row=18
// diagnostic 2: UIPasteboard.general.string = message fixture=2 row=19
// diagnostic 2: clipboard.writeText(result); fixture=2 row=20
// diagnostic 2: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=2 row=21
// diagnostic 2: user@localhost @mentions npm @scope/pkg fixture=2 row=22
// diagnostic 2: clipboard_label: "Copy item" fixture=2 row=23
// diagnostic 2: clipboard_enabled = false fixture=2 row=24
// diagnostic 2: clipboard_count = 12 fixture=2 row=25
// diagnostic 2: document.execCommand("copy") fixture=2 row=26
// diagnostic 2: error E0123 at module.rs:42:8 fixture=2 row=27
// diagnostic 2: release checksum=abcdef0123456789 fixture=2 row=28
// diagnostic 2: duration=00:12:34.567 fixture=2 row=29
// diagnostic 2: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=2 row=30
// diagnostic 2: sha256=faabababababababababababababababababababababababababababababababa fixture=2 row=31
// diagnostic 2: listener=192.0.2.8 upstream=2001:db8::42 fixture=2 row=32
// diagnostic 2: version=1.25.0 build=v2026.10.27 fixture=2 row=33
// diagnostic 2: timestamp=2026-10-27T12:34:56.789Z fixture=2 row=34
// diagnostic 2: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=2 row=35
// diagnostic 2: invalid_luhn=4242-4242-4242-4241 fixture=2 row=36
// diagnostic 2: invalid_luhn=4111111111111112 fixture=2 row=37
// diagnostic 2: -----BEGIN PUBLIC KEY----- fixture=2 row=38
// diagnostic 2: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=2 row=39

"""
