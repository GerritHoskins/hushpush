val fixture = """
// diagnostic 8: "dev-tools@never.invalid".
// diagnostic 8: order=ORD-QX-108 date=2026-10-27 card=[3714 4963 5398 431], amount=EUR 19.95
// diagnostic 8: contact: (+39 06 5550 1234);
// diagnostic 8: local_contact="06 5550 1234".
// diagnostic 8: international_national_format: +39 06 5550 1234
// diagnostic 8: open: /home/sam.dev/src/app
// diagnostic 8: -----BEGIN RSA PRIVATE KEY-----
Proc-Type: 4,ENCRYPTED
DEK-Info: AES-128-CBC,00112233445566778899AABBCCDDEEFF

OVUtrHZLEicq5ZKDIeppnf+xL7nFYWp7HanGt12jU15D8QVGxVHqo4wzzFXnESt4
S4bN0b18Tv6u33GSIJ8XeDMYGU1fAnuI9IeioLlEjoi5HUIukk3N4t4ctr+EB6mF
-----END RSA PRIVATE KEY-----
// diagnostic 8: [2026-10-27 12:34:56] Clipboard history entry 8
	A quiet lantern marks the rehearsal entrance.
	Keep the paper rehearsal schedule nearby.
// diagnostic 8: reviewer: <Orion Wren>;
// diagnostic 8: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=8 row=0
// diagnostic 8: sha256=faabababababababababababababababababababababababababababababababa fixture=8 row=1
// diagnostic 8: listener=192.0.2.8 upstream=2001:db8::42 fixture=8 row=2
// diagnostic 8: version=1.25.0 build=v2026.10.27 fixture=8 row=3
// diagnostic 8: timestamp=2026-10-27T12:34:56.789Z fixture=8 row=4
// diagnostic 8: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=8 row=5
// diagnostic 8: invalid_luhn=4242-4242-4242-4241 fixture=8 row=6
// diagnostic 8: invalid_luhn=4111111111111112 fixture=8 row=7
// diagnostic 8: -----BEGIN PUBLIC KEY----- fixture=8 row=8
// diagnostic 8: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=8 row=9
// diagnostic 8: -----END PUBLIC KEY----- fixture=8 row=10
// diagnostic 8: -----BEGIN CERTIFICATE----- fixture=8 row=11
// diagnostic 8: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=8 row=12
// diagnostic 8: -----END CERTIFICATE----- fixture=8 row=13
// diagnostic 8: reference 555-0200 and 555-9999 are extension tokens fixture=8 row=14
// diagnostic 8: i18n.en.clipboard = "Copy to clipboard" fixture=8 row=15
// diagnostic 8: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=8 row=16
// diagnostic 8: await navigator.clipboard.readText(); fixture=8 row=17
// diagnostic 8: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=8 row=18
// diagnostic 8: UIPasteboard.general.string = message fixture=8 row=19
// diagnostic 8: clipboard.writeText(result); fixture=8 row=20
// diagnostic 8: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=8 row=21
// diagnostic 8: user@localhost @mentions npm @scope/pkg fixture=8 row=22
// diagnostic 8: clipboard_label: "Copy item" fixture=8 row=23
// diagnostic 8: clipboard_enabled = false fixture=8 row=24
// diagnostic 8: clipboard_count = 12 fixture=8 row=25
// diagnostic 8: document.execCommand("copy") fixture=8 row=26
// diagnostic 8: error E0123 at module.rs:42:8 fixture=8 row=27
// diagnostic 8: release checksum=abcdef0123456789 fixture=8 row=28
// diagnostic 8: duration=00:12:34.567 fixture=8 row=29
// diagnostic 8: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=8 row=30
// diagnostic 8: sha256=faabababababababababababababababababababababababababababababababa fixture=8 row=31
// diagnostic 8: listener=192.0.2.8 upstream=2001:db8::42 fixture=8 row=32
// diagnostic 8: version=1.25.0 build=v2026.10.27 fixture=8 row=33
// diagnostic 8: timestamp=2026-10-27T12:34:56.789Z fixture=8 row=34
// diagnostic 8: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=8 row=35
// diagnostic 8: invalid_luhn=4242-4242-4242-4241 fixture=8 row=36
// diagnostic 8: invalid_luhn=4111111111111112 fixture=8 row=37
// diagnostic 8: -----BEGIN PUBLIC KEY----- fixture=8 row=38
// diagnostic 8: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=8 row=39

"""
