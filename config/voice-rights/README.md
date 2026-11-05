# Voice rights records

One JSON file per voice that the project trains itself, named `<voiceId>.json`, following `config/voice-rights.schema.json`. A catalogue entry for such a voice must name its record in `rightsRecord`. A test (`core/tests/voice_rights.rs`) and the voice tools (`tools/khmer-voice/`) refuse a voice whose record is missing, not `granted`, or incomplete.

What counts as permission:

- **Signed licence, contract or written email** from every rights holder (the station or producer that owns the recordings, and the person who speaks). It must allow training a model on the recordings and shipping the model in a product. Keep the document under `docs/voice-rights/` (not in the repository if it is private; then name where it is kept).
- **Open licence** of a dataset (for example CC-BY or CC-BY-SA), with the licence text. Share-alike licences make the voice `copyleftFile` with a visible notice; non-commercial ones make it `nonCommercial` and opt-in.

What does **not** count: recordings being easy to find or hear, being broadcast on radio or posted online, or "public" in everyday speech. A recording that is public is not licence-free, and a person's voice is theirs (Constitution VII). `assumed-public` is accepted by the schema only so the test can reject it.

Output of a cloud voice service is never training data: provider terms forbid building a competing voice from it.
