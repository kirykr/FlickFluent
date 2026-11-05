# Feature Specification: Hybrid Cloud Engines and API Keys

**Feature Branch**: `003-hybrid-cloud-keys`

**Created**: 2026-11-05

**Status**: Draft

**Input**: User description: "Hybrid cloud engines and API keys. Machines below the full-local spec can use optional cloud engines for heavy steps (translation, speech) with the user's own API keys. Keys are stored in the OS keychain, never in plain text. Cloud steps are off by default, clearly labelled, and show what will be sent before anything leaves the machine."

Covers master-spec Story 8 (the cloud key and engine part), FR-021, FR-022, FR-023 and FR-031. Builds on feature 002 (local dub pipeline, model catalogue, language pair check, hardware check).

## Clarifications

### Session 2026-11-05

- Q: When a cloud service offers both translation and speech, does one key serve both steps, or one key per step? → A: One key per service, usable for every step that service offers. Order, consent and engine choice stay per step.
- Q: How long does the app wait on its own when every key is rate-limited? → A: Up to 10 minutes in total per request, then the run pauses in a resumable state.
- Q: Does the app ask again when a run will send much more text than the user agreed to? → A: It shows the amount before every cloud run, and asks again only if it is more than 5 times the amount last agreed to.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Add a key and check that it works (Priority: P1)

A user whose computer is below the full-local spec wants better translation or speech than the small local models give. They open Settings, choose a cloud service, paste their own key (one key covers every step that service offers), and press "Verify connection". The app says the key works, or says exactly why not (wrong key, no credit, service down, no internet). The key is kept safe by the computer's own secure storage and is never shown in full again.

**Why this priority**: Nothing else in this feature works without a stored, verified key. It is the smallest slice that can be tested alone and it is the "Independent Test" of master-spec Story 8.

**Independent Test**: Add a key, press "Verify connection", get a success message; restart the app and see the key listed as saved (masked) without being able to read it back; remove it and see it gone.

**Acceptance Scenarios**:

1. **Given** a valid key for a service, **When** the user presses "Verify connection", **Then** the app reports success and names the service.
2. **Given** a wrong key, **When** the user verifies it, **Then** the app reports that the service rejected the key, and does not save it as working.
3. **Given** a saved key, **When** the user reopens the app or the Settings screen, **Then** the key shows only as masked (for example the last four characters) and the full value cannot be read or copied from the app.
4. **Given** the computer has no internet, **When** the user verifies a key, **Then** the app says it could not reach the service, and the saved key is kept.
5. **Given** a saved key, **When** the user removes it, **Then** it is deleted from the computer's secure storage and no cloud step can use it.

---

### User Story 2 - Dub with a cloud step, knowing what leaves the computer (Priority: P1)

A user on a hybrid-mode computer starts a dub. For the heavy step the small local model cannot do well or at all (for example translating into a language the local model covers poorly), the app offers the cloud engine they set up. Before the first use, a plain notice says which service gets what: the text lines to translate, or the translated text to speak, and that no video and no original audio is sent. The user agrees or declines. The cloud steps are clearly labelled in the run, and the finished dub works the same as a local one.

**Why this priority**: This is the point of hybrid mode (FR-021, FR-022) and the privacy promise (Constitution IX). Without the consent step, the feature must not ship.

**Independent Test**: With a verified key, start a dub that needs the cloud step, read the notice, agree, and see the run finish with the cloud step labelled; repeat and decline, and see that nothing was sent and the run either uses the local fallback or stops with a clear next step.

**Acceptance Scenarios**:

1. **Given** cloud engines are off (the default), **When** the user runs any dub, **Then** nothing is sent to any service.
2. **Given** a verified key and a dub that would use a cloud step, **When** the user starts the run for the first time with that service, **Then** the app shows which service, which step, and the kind and amount of data that will be sent, and waits for the user to agree.
3. **Given** the user declines, **When** a local option exists for the step, **Then** the run offers it; **otherwise** the run does not start and says why and what to do.
4. **Given** the user agreed, **When** the run is shown, **Then** each cloud step is labelled as cloud, with the service name, in the stage list and in the finished project's details.
5. **Given** the user agreed once for a service and step, **When** they later start another dub, **Then** the app shows the amount of text that run will send and labels the cloud step, but does not ask again unless the amount is more than 5 times the amount last agreed to; the user can withdraw consent in Settings.
6. **Given** a cloud step, **When** data is sent, **Then** only text for that step is sent; never the video, never the original audio, never other projects.

---

### User Story 3 - Several keys, rate limits and failures without losing work (Priority: P2)

A user adds more than one key for the same service (for example two accounts at one translation service) because one key's rate limit is too low for a whole film. During a long run, when the service answers "too many requests" or a key fails, the app switches to the next key and carries on from where it stopped. If every key is limited or failing, the run waits and retries, or tells the user how long to wait, and keeps all finished work.

**Why this priority**: Long films hit rate limits; losing a run to one would make the cloud option unusable (FR-031, Story 8 scenario 2). It depends on Story 1, and Story 2's flow, but is a separate, testable behaviour.

**Independent Test**: Give the app two keys where the first is made to fail partway; confirm the run finishes using the second, with no line translated twice or missing, and the key order and which key was used are visible afterwards.

**Acceptance Scenarios**:

1. **Given** two keys for the same service, **When** the first is rate-limited mid-run, **Then** the run continues on the second from the next unfinished line, and finished lines are not redone.
2. **Given** a key that fails for a reason that will not go away (revoked, no credit), **When** it fails, **Then** it is set aside for the rest of the run and the user is told which key and why.
3. **Given** every key is rate-limited, **When** the service says how long to wait, **Then** the app waits that long and retries, up to 10 minutes in total for one request; past that it pauses the run, tells the user how long the service asked for, and keeps the run resumable.
4. **Given** a run stopped while waiting, **When** the user closes and reopens the app and resumes, **Then** it continues from the last finished line.
5. **Given** a key that works again after its limit passes, **When** another run starts, **Then** it is tried again from the first key in the user's order.

---

### User Story 4 - Choose local or cloud per step (Priority: P2)

In Settings, for each heavy step (translation, speech) the user sees which engine is used now: local, or a named cloud service. They can switch a step between local and cloud, put their keys in order, see how much text a run sent and to whom, and withdraw consent. On a computer that can run everything locally, cloud is still available as a choice but never the default.

**Why this priority**: Completes master-spec Story 8's "voice and translation engine (local or cloud)". Useful, but a user can already use cloud through Stories 1 to 3 without it.

**Independent Test**: Switch translation to a cloud service, run a dub and see it labelled cloud; switch back to local, run again and see nothing sent.

**Acceptance Scenarios**:

1. **Given** a fresh install, **When** the user opens the engine settings, **Then** every step shows "Local" and every cloud option is off.
2. **Given** a user who switches a step to cloud without a verified key, **When** they confirm, **Then** the app asks them to add and verify a key first.
3. **Given** a finished project that used a cloud step, **When** the user opens its details, **Then** it lists which steps used which service and how much text was sent.
4. **Given** the user withdraws consent for a service, **When** they start a dub, **Then** the app asks again before sending anything.

---

### User Story 5 - Hybrid-mode guidance when the computer is too weak for a step (Priority: P3)

When the check before a run finds that this computer cannot run a heavy step locally for the chosen languages, the message that blocks the run now also offers the cloud engine as a way forward (if a key exists) or tells the user how to set one up (if not), alongside the existing fallbacks such as subtitles only.

**Why this priority**: Makes hybrid mode discoverable; the earlier stories already work if the user goes to Settings by themselves.

**Independent Test**: On a pair the local models cannot do on this machine, see the cloud engine offered in the fallback list; choose it and the run starts after the consent notice.

**Acceptance Scenarios**:

1. **Given** a language pair where a heavy step cannot run on this computer, **When** the pair check runs, **Then** the fallback list includes "use the cloud service" if a verified key covers that pair, or "set up a cloud key" if not.
2. **Given** a cloud engine that supports a language the local models do not, **When** the language lists are built, **Then** that language appears for the user once the engine is set up, with no change to the screens.

---

### Edge Cases

- **Keychain unavailable or locked**: the app refuses to store the key (never falls back to a plain file) and says so; the user may use the key for the current session only.
- **Key pasted with spaces or line breaks**: stripped before saving and verifying.
- **Verification passes but a later call fails**: handled as a failure of that key (Story 3), not as "verified means always works".
- **Service changes its limits or model list**: the engine list is data, so a changed list needs no screen change; an unknown reply is reported as "unexpected answer from the service" with the service's own message.
- **Text sent contains the user's own words**: the notice says the text of the film's speech is sent, since this is what the step is.
- **Partial consent**: the user agrees for translation but not for speech; only translation may use the cloud.
- **Offline during a run**: the cloud step pauses with a clear message and the run can be resumed or switched to a local option; finished work stays, except that switching the engine of a step redoes that step.
- **Cost**: the app shows the amount of text a run will send before it starts, but the user's bill is between them and the service; the app does not claim a price it cannot know.
- **Same key added twice**: rejected as a duplicate.
- **A project is shared**: keys are never saved inside a project file; consent records are not carried to another computer.
- **Windows and macOS**: the same behaviour on both; each uses its own secure storage.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST let the user add, verify, reorder and remove their own API keys for cloud services. A key belongs to one service and MUST be usable for every step that service offers (translation, speech); the user does not enter it again per step. A service may have several keys. (Master FR-021, FR-031)
- **FR-002**: "Verify connection" MUST report success, or the exact failure reason in plain words (rejected key, no credit or quota, service unreachable, no internet, unexpected answer). (Story 8)
- **FR-003**: Keys MUST be stored only in the operating system's secure storage and MUST NOT appear in plain text in files, project files, logs, error messages, crash reports or exports. (Master FR-023, Constitution IX)
- **FR-004**: The app MUST show a saved key only in masked form and MUST NOT offer any way to read the full key back.
- **FR-005**: All cloud engines MUST be off by default. A cloud step MUST run only after the user has turned the engine on and agreed to the data notice for that service and step. (Master FR-022)
- **FR-006**: Before the first use of a service for a step, the app MUST show a notice naming the service, the step, what is sent (text of the lines for that step, with the approximate amount), what is not sent (video, original audio, other projects), and that the service's own terms apply. (Master FR-022)
- **FR-007**: The user MUST be able to agree per service and per step, and to withdraw that agreement at any time in Settings. After withdrawal the notice MUST be shown again before any data is sent. The agreement records the amount of text it was given for. Every later run that uses the step MUST show its own amount before it starts, and MUST ask again if that amount is more than 5 times the recorded one; a new agreement replaces the recorded amount.
- **FR-008**: Cloud steps MUST be labelled as cloud, with the service name, wherever the step appears: the engine settings, the run's stage list, and the finished project's details.
- **FR-009**: The app MUST NOT send any data to a cloud service for a step the user has not turned on, and MUST NOT switch a step from local to cloud on its own. Switching from cloud to local is allowed only with the user's choice.
- **FR-010**: Only text needed for the step MAY be sent. The video file and the original audio MUST never be sent by this feature.
- **FR-011**: The app MUST accept several keys for the same service, try them in the user's order, and move to the next on a rate limit or a failure, retrying the same work without redoing finished lines. Keys of one service are never replaced by another service's key during a run, because the user agreed to send data to that service only. (Master FR-031)
- **FR-012**: On a rate limit the app MUST wait for the time the service states (or 30 seconds when it states none) and retry, for up to 10 minutes in total for one request. After that it MUST pause the run and tell the user how long to wait. In every case finished work MUST be kept and the run MUST be resumable. (Story 8 scenario 2)
- **FR-013**: A key that fails for a lasting reason MUST be set aside for the rest of the run and named to the user with the reason; other keys MUST keep working.
- **FR-014**: When no key can serve a request, the run MUST stop in a resumable state with a next step (wait, add a key, use a local option if the computer can).
- **FR-015**: The language pair check from feature 002 MUST include cloud engines as a fallback when a verified key covers the pair, and as a "set up a key" hint when not. Languages offered by a cloud engine MUST come from data, not from changes to screens. (Constitution IV)
- **FR-016**: The engine settings MUST show, per step, the engine in use and let the user switch between local and any cloud engine that has a verified key.
- **FR-017**: A finished project MUST record which steps used which service and how much text was sent, without storing any key.
- **FR-018**: The feature MUST work the same on Windows and macOS, using each system's own secure storage. If the secure storage cannot be used, the app MUST refuse to save the key and offer to use it for the current session only. (Constitution I, IX)
- **FR-019**: A key MUST be removable at any time, deleting it from secure storage, and a removed key MUST NOT be used by any later step.
- **FR-020**: Errors from services MUST be shown in plain words with a next step, and MUST NOT include the key.

### Key Entities

- **Cloud service**: a kind of engine the user can choose (what it does, which languages and voices it offers, what data it needs, a link to its terms). Described in data so adding one is not a screen change.
- **Cloud key**: one secret the user owns for a service; it serves every step that service offers. Has a label (user's own name), a masked display value, an order, a state (untested, verified, set aside this run) and when it was last verified. The secret itself exists only in the computer's secure storage.
- **Consent record**: the user's agreement, for one service and one step (a key for the service does not imply agreement for every step), to send a stated kind of data in a stated amount. Has a date and can be withdrawn. Kept on this computer only.
- **Engine choice**: for each heavy step, which engine is in use (local or one named cloud service).
- **Usage record**: per project and step, the service used, how much text was sent, how many keys were tried, and whether any retries or switches happened.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can add a key and see "Verify connection" succeed or fail with a reason in under 30 seconds on a normal connection.
- **SC-002**: After adding a key, searching every file the app writes (settings, projects, logs, exports) for the key's text finds nothing, on both Windows and macOS.
- **SC-003**: With cloud engines off (the default), a full dub produces zero outgoing requests to any cloud service.
- **SC-004**: In 100% of runs that use a cloud step, the data notice was shown and agreed to before the first request, and the step is labelled as cloud in the run and in the project.
- **SC-005**: With two keys for one service where the first is limited partway through a 30-minute film, the run finishes using the second with no line missing or translated twice, and no more than one line's work repeated.
- **SC-006**: When every key is limited, no finished work is lost, and the user is told how long to wait or can resume later, in 100% of tested cases.
- **SC-007**: A user who has never used a cloud engine can switch one translation step from local to cloud and back, and see the correct label each time, without help, within 3 minutes.
- **SC-008**: A language that only a cloud engine offers appears in the language list after setup with no change to the screens (checked by adding a data entry).

## Assumptions

- **Scope of steps**: translation and speech are the cloud steps in this feature. Voice cloning belongs to feature 005 and cloud transcription is left for later; the data model leaves room for both.
- **Providers**: the first release supports a small set of well-known services chosen in planning (at least one for translation and one for speech), described as data. The set is not fixed by this spec.
- **Who can use it**: any user can turn on a cloud engine, whatever their computer; hybrid-mode computers are guided to it (Story 5), and full-local computers are never nudged toward it.
- **Data sent**: text only, as the steps need: the lines to translate, or the translated lines to speak. The video, the original audio and other projects are never sent.
- **Billing**: keys are the user's own, so usage is billed by the service to the user. The app shows the amount of text sent but does not promise a price.
- **Consent scope**: consent is per service and per step, kept on this computer, and not copied into project files.
- **Secure storage**: Windows and macOS each have an operating-system store for secrets. If it is unavailable, the key is not saved (session only). Linux, web and mobile come later with the platform roadmap.
- **Existing behaviour**: feature 002's local pipeline, resume, progress and error display are reused; a cloud step uses the same section-by-section progress and resumable work.
- **Out of scope**: sign-in accounts, the app selling or proxying cloud usage, voice cloning, cloud transcription, and any use of keys by other features.
