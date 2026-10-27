const FIXTURE: &str = r###"
// diagnostic 0: https://portal.test/member/a+b@deep.sub.example.com?view=brief
// diagnostic 0: order=ORD-QX-100 date=2026-10-27 card=[2223003122003222], amount=EUR 19.95
// diagnostic 0: contact: (+1 202 555 0134);
// diagnostic 0: local_contact="202-555-0134".
// diagnostic 0: international_national_format: +81 3 5550 1234
// diagnostic 0: open: /Users/ada/work/report.log
// diagnostic 0: -----BEGIN PRIVATE KEY-----
UuKEGwL9gd089SbgDtcGCX8GLUc/NSF4FRjrtRl/2AI3Eg2Li90W54FmERbhAiQm
IYxKxrPPuxwXP3K69b6aeYae1JsKILiBFAv5xu8v/p9d5fTFdemVSG8PMhuGwb8t
// diagnostic 0: [2026-10-27T12:34:56Z] Clipboard history entry 0
    Please leave three paper cranes beside the archive.
    Keep the paper rehearsal schedule nearby.
// diagnostic 0: reviewer: <Coralie Fable>;
// diagnostic 0: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=0 row=0
// diagnostic 0: sha256=faabababababababababababababababababababababababababababababababa fixture=0 row=1
// diagnostic 0: listener=192.0.2.8 upstream=2001:db8::42 fixture=0 row=2
// diagnostic 0: version=1.25.0 build=v2026.10.27 fixture=0 row=3
// diagnostic 0: timestamp=2026-10-27T12:34:56.789Z fixture=0 row=4
// diagnostic 0: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=0 row=5
// diagnostic 0: invalid_luhn=4242-4242-4242-4241 fixture=0 row=6
// diagnostic 0: invalid_luhn=4111111111111112 fixture=0 row=7
// diagnostic 0: -----BEGIN PUBLIC KEY----- fixture=0 row=8
// diagnostic 0: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=0 row=9
// diagnostic 0: -----END PUBLIC KEY----- fixture=0 row=10
// diagnostic 0: -----BEGIN CERTIFICATE----- fixture=0 row=11
// diagnostic 0: U1lOVEhFVElDLUNFUlRJRklDQVRF fixture=0 row=12
// diagnostic 0: -----END CERTIFICATE----- fixture=0 row=13
// diagnostic 0: reference 555-0200 and 555-9999 are extension tokens fixture=0 row=14
// diagnostic 0: i18n.en.clipboard = "Copy to clipboard" fixture=0 row=15
// diagnostic 0: i18n.de.paste = "Aus Zwischenablage einfügen" fixture=0 row=16
// diagnostic 0: await navigator.clipboard.readText(); fixture=0 row=17
// diagnostic 0: ClipboardManager.setPrimaryClip(ClipData.newPlainText(label, text)); fixture=0 row=18
// diagnostic 0: UIPasteboard.general.string = message fixture=0 row=19
// diagnostic 0: clipboard.writeText(result); fixture=0 row=20
// diagnostic 0: path=$HOME/work path=%USERPROFILE%/work path=/home/runner/work fixture=0 row=21
// diagnostic 0: user@localhost @mentions npm @scope/pkg fixture=0 row=22
// diagnostic 0: clipboard_label: "Copy item" fixture=0 row=23
// diagnostic 0: clipboard_enabled = false fixture=0 row=24
// diagnostic 0: clipboard_count = 12 fixture=0 row=25
// diagnostic 0: document.execCommand("copy") fixture=0 row=26
// diagnostic 0: error E0123 at module.rs:42:8 fixture=0 row=27
// diagnostic 0: release checksum=abcdef0123456789 fixture=0 row=28
// diagnostic 0: duration=00:12:34.567 fixture=0 row=29
// diagnostic 0: trace_id=123e4567-e89b-42d3-a456-426614174000 fixture=0 row=30
// diagnostic 0: sha256=faabababababababababababababababababababababababababababababababa fixture=0 row=31
// diagnostic 0: listener=192.0.2.8 upstream=2001:db8::42 fixture=0 row=32
// diagnostic 0: version=1.25.0 build=v2026.10.27 fixture=0 row=33
// diagnostic 0: timestamp=2026-10-27T12:34:56.789Z fixture=0 row=34
// diagnostic 0: order=ORD-QX-314159 tracking=TRACK-ZK-092AB fixture=0 row=35
// diagnostic 0: invalid_luhn=4242-4242-4242-4241 fixture=0 row=36
// diagnostic 0: invalid_luhn=4111111111111112 fixture=0 row=37
// diagnostic 0: -----BEGIN PUBLIC KEY----- fixture=0 row=38
// diagnostic 0: U1lOVEhFVElDLVBVQkxJQy1PTkxZ fixture=0 row=39

"###;
