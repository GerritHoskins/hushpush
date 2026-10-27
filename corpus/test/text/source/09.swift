let fixture = """
// diagnostic 9: owner='robot.42@unit.example';
// diagnostic 9: order=ORD-QX-109 date=2026-10-27 card=[5555555555554444], amount=EUR 19.95
// diagnostic 9: contact: (+61 2 5550 1234);
// diagnostic 9: local_contact="02 5550 1234".
// diagnostic 9: international_national_format: +61 2 5550 1234
// diagnostic 9: open: C:/Users/person42/work/notes.txt
// diagnostic 9: private_key: |
    -----BEGIN EC PRIVATE KEY-----
    ppgf/qqwCNBZXXAKR0lmSo8R6Jbvdz6a9xq1O2+De1tap6McOC+R/nQNVpcKSb3g
    7AIOOqoKza5SpPYSq0F+EixLEkFLVGu5HDeq7FMOhqcO/1R0Fz4l1XfchwJ8JaIp
    -----END EC PRIVATE KEY-----
// diagnostic 9: ClipboardService: ClipData { text/plain "The rehearsal starts after the brass bell rings." }
// diagnostic 9: reviewer: <Coralie Fable>;
// diagnostic 9: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=9 row=0
// diagnostic 9: sha256=faabababababababababababababababababababababababababababababababa fixture=9 row=1
// diagnostic 9: listener=192.0.2.8 upstream=2001:db8::42 fixture=9 row=2
// diagnostic 9: version=1.25.0 build=v2026.10.27 fixture=9 row=3
// diagnostic 9: timestamp=2026-10-27T12:34:56.789Z fixture=9 row=4
// diagnostic 9: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=9 row=5
// diagnostic 9: invalid_luhn=4242-4242-4242-4241 fixture=9 row=6
// diagnostic 9: invalid_luhn=4111111111111112 fixture=9 row=7
// diagnostic 9: -----BEGIN PUBLIC KEY----- fixture=9 row=8
// diagnostic 9: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=9 row=9
// diagnostic 9: -----END PUBLIC KEY----- fixture=9 row=10
// diagnostic 9: -----BEGIN CERTIFICATE----- fixture=9 row=11
// diagnostic 9: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=9 row=12
// diagnostic 9: -----END CERTIFICATE----- fixture=9 row=13
// diagnostic 9: reference 555-0200 and 555-9999 are extension tokens fixture=9 row=14
// diagnostic 9: i18n.en.clipboard = "Copy to clipboard" fixture=9 row=15
// diagnostic 9: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=9 row=16
// diagnostic 9: await navigator.clipboard.readText(); fixture=9 row=17
// diagnostic 9: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=9 row=18
// diagnostic 9: UIPasteboard.general.string = message fixture=9 row=19
// diagnostic 9: clipboard.writeText(result); fixture=9 row=20
// diagnostic 9: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=9 row=21
// diagnostic 9: user@localhost @mentions npm @scope/pkg fixture=9 row=22
// diagnostic 9: clipboard_label: "Copy item" fixture=9 row=23
// diagnostic 9: clipboard_enabled = false fixture=9 row=24
// diagnostic 9: clipboard_count = 12 fixture=9 row=25
// diagnostic 9: document.execCommand("copy") fixture=9 row=26
// diagnostic 9: error E0123 at module.rs:42:8 fixture=9 row=27
// diagnostic 9: release checksum=abcdef0123456789 fixture=9 row=28
// diagnostic 9: duration=00:12:34.567 fixture=9 row=29
// diagnostic 9: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=9 row=30
// diagnostic 9: sha256=faabababababababababababababababababababababababababababababababa fixture=9 row=31
// diagnostic 9: listener=192.0.2.8 upstream=2001:db8::42 fixture=9 row=32
// diagnostic 9: version=1.25.0 build=v2026.10.27 fixture=9 row=33
// diagnostic 9: timestamp=2026-10-27T12:34:56.789Z fixture=9 row=34
// diagnostic 9: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=9 row=35
// diagnostic 9: invalid_luhn=4242-4242-4242-4241 fixture=9 row=36
// diagnostic 9: invalid_luhn=4111111111111112 fixture=9 row=37
// diagnostic 9: -----BEGIN PUBLIC KEY----- fixture=9 row=38
// diagnostic 9: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=9 row=39

"""
