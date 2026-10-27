val fixture = """
// diagnostic 3: "test-user@ci.example.org".
// diagnostic 3: order=ORD-QX-103 date=2026-10-27 card=[378282246310005], amount=EUR 19.95
// diagnostic 3: contact: (+44 1632 960123);
// diagnostic 3: local_contact="01632 960123".
// diagnostic 3: international_national_format: +44 1632 960123
// diagnostic 3: open: C:/Users/person42/work/notes.txt
// diagnostic 3: -----BEGIN EC PRIVATE KEY-----
VhxB3dV/X7wvKdhSMRg+jGmqMNj/YdTXXdxvEtL82LaVK9t5vjDDKDAXjI2B/pOk
U+6xTLz7FfelUWL7Ov/Bf+GJCURIpiale7BTniKe6LRLpLYIZcFM+k/ShhadCVQ9
-----END EC PRIVATE KEY-----
// diagnostic 3: ClipboardService: ClipData { text/plain "A quiet lantern marks the rehearsal entrance." }
// diagnostic 3: reviewer: <Coralie Fable>;
// diagnostic 3: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=3 row=0
// diagnostic 3: sha256=faabababababababababababababababababababababababababababababababa fixture=3 row=1
// diagnostic 3: listener=192.0.2.8 upstream=2001:db8::42 fixture=3 row=2
// diagnostic 3: version=1.25.0 build=v2026.10.27 fixture=3 row=3
// diagnostic 3: timestamp=2026-10-27T12:34:56.789Z fixture=3 row=4
// diagnostic 3: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=3 row=5
// diagnostic 3: invalid_luhn=4242-4242-4242-4241 fixture=3 row=6
// diagnostic 3: invalid_luhn=4111111111111112 fixture=3 row=7
// diagnostic 3: -----BEGIN PUBLIC KEY----- fixture=3 row=8
// diagnostic 3: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=3 row=9
// diagnostic 3: -----END PUBLIC KEY----- fixture=3 row=10
// diagnostic 3: -----BEGIN CERTIFICATE----- fixture=3 row=11
// diagnostic 3: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=3 row=12
// diagnostic 3: -----END CERTIFICATE----- fixture=3 row=13
// diagnostic 3: reference 555-0200 and 555-9999 are extension tokens fixture=3 row=14
// diagnostic 3: i18n.en.clipboard = "Copy to clipboard" fixture=3 row=15
// diagnostic 3: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=3 row=16
// diagnostic 3: await navigator.clipboard.readText(); fixture=3 row=17
// diagnostic 3: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=3 row=18
// diagnostic 3: UIPasteboard.general.string = message fixture=3 row=19
// diagnostic 3: clipboard.writeText(result); fixture=3 row=20
// diagnostic 3: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=3 row=21
// diagnostic 3: user@localhost @mentions npm @scope/pkg fixture=3 row=22
// diagnostic 3: clipboard_label: "Copy item" fixture=3 row=23
// diagnostic 3: clipboard_enabled = false fixture=3 row=24
// diagnostic 3: clipboard_count = 12 fixture=3 row=25
// diagnostic 3: document.execCommand("copy") fixture=3 row=26
// diagnostic 3: error E0123 at module.rs:42:8 fixture=3 row=27
// diagnostic 3: release checksum=abcdef0123456789 fixture=3 row=28
// diagnostic 3: duration=00:12:34.567 fixture=3 row=29
// diagnostic 3: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=3 row=30
// diagnostic 3: sha256=faabababababababababababababababababababababababababababababababa fixture=3 row=31
// diagnostic 3: listener=192.0.2.8 upstream=2001:db8::42 fixture=3 row=32
// diagnostic 3: version=1.25.0 build=v2026.10.27 fixture=3 row=33
// diagnostic 3: timestamp=2026-10-27T12:34:56.789Z fixture=3 row=34
// diagnostic 3: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=3 row=35
// diagnostic 3: invalid_luhn=4242-4242-4242-4241 fixture=3 row=36
// diagnostic 3: invalid_luhn=4111111111111112 fixture=3 row=37
// diagnostic 3: -----BEGIN PUBLIC KEY----- fixture=3 row=38
// diagnostic 3: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=3 row=39

"""
