FIXTURE = r"""
// diagnostic 7: "dev-tools@never.invalid".
// diagnostic 7: order=ORD-QX-107 date=2026-10-27 card=[3714 4963 5398 431], amount=EUR 19.95
// diagnostic 7: contact: (+39 06 5550 1234);
// diagnostic 7: local_contact="06 5550 1234".
// diagnostic 7: international_national_format: +39 06 5550 1234
// diagnostic 7: open: /home/sam.dev/src/app
// diagnostic 7: -----BEGIN RSA PRIVATE KEY-----
Proc-Type: 4,ENCRYPTED
DEK-Info: AES-128-CBC,00112233445566778899AABBCCDDEEFF

p5PAgks5898+Dumgj2ROlqW6MWjycxYC91Cif7mBr50ecb3bwIkbviyOLe+mcrYq
05kKJQP2g/nfkLCntUUJxcjTJHgyjr4xVkgG5fVHBLdOVRU59U25zjSkScou1Eta
-----END RSA PRIVATE KEY-----
// diagnostic 7: [2026-10-27 12:34:56] Clipboard history entry 7
	A quiet lantern marks the rehearsal entrance.
	Keep the paper rehearsal schedule nearby.
// diagnostic 7: reviewer: <Orion Wren>;
// diagnostic 7: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=7 row=0
// diagnostic 7: sha256=faabababababababababababababababababababababababababababababababa fixture=7 row=1
// diagnostic 7: listener=192.0.2.8 upstream=2001:db8::42 fixture=7 row=2
// diagnostic 7: version=1.25.0 build=v2026.10.27 fixture=7 row=3
// diagnostic 7: timestamp=2026-10-27T12:34:56.789Z fixture=7 row=4
// diagnostic 7: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=7 row=5
// diagnostic 7: invalid_luhn=4242-4242-4242-4241 fixture=7 row=6
// diagnostic 7: invalid_luhn=4111111111111112 fixture=7 row=7
// diagnostic 7: -----BEGIN PUBLIC KEY----- fixture=7 row=8
// diagnostic 7: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=7 row=9
// diagnostic 7: -----END PUBLIC KEY----- fixture=7 row=10
// diagnostic 7: -----BEGIN CERTIFICATE----- fixture=7 row=11
// diagnostic 7: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=7 row=12
// diagnostic 7: -----END CERTIFICATE----- fixture=7 row=13
// diagnostic 7: reference 555-0200 and 555-9999 are extension tokens fixture=7 row=14
// diagnostic 7: i18n.en.clipboard = "Copy to clipboard" fixture=7 row=15
// diagnostic 7: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=7 row=16
// diagnostic 7: await navigator.clipboard.readText(); fixture=7 row=17
// diagnostic 7: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=7 row=18
// diagnostic 7: UIPasteboard.general.string = message fixture=7 row=19
// diagnostic 7: clipboard.writeText(result); fixture=7 row=20
// diagnostic 7: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=7 row=21
// diagnostic 7: user@localhost @mentions npm @scope/pkg fixture=7 row=22
// diagnostic 7: clipboard_label: "Copy item" fixture=7 row=23
// diagnostic 7: clipboard_enabled = false fixture=7 row=24
// diagnostic 7: clipboard_count = 12 fixture=7 row=25
// diagnostic 7: document.execCommand("copy") fixture=7 row=26
// diagnostic 7: error E0123 at module.rs:42:8 fixture=7 row=27
// diagnostic 7: release checksum=abcdef0123456789 fixture=7 row=28
// diagnostic 7: duration=00:12:34.567 fixture=7 row=29
// diagnostic 7: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=7 row=30
// diagnostic 7: sha256=faabababababababababababababababababababababababababababababababa fixture=7 row=31
// diagnostic 7: listener=192.0.2.8 upstream=2001:db8::42 fixture=7 row=32
// diagnostic 7: version=1.25.0 build=v2026.10.27 fixture=7 row=33
// diagnostic 7: timestamp=2026-10-27T12:34:56.789Z fixture=7 row=34
// diagnostic 7: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=7 row=35
// diagnostic 7: invalid_luhn=4242-4242-4242-4241 fixture=7 row=36
// diagnostic 7: invalid_luhn=4111111111111112 fixture=7 row=37
// diagnostic 7: -----BEGIN PUBLIC KEY----- fixture=7 row=38
// diagnostic 7: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=7 row=39

"""
