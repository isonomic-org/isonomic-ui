# Kits deviations

Each place where code for the tool kits differs from its reference: what
changed, why, and the reference file. The design calls this file the record
of deviations. A deviation without an entry here is a defect.

## Permission prompt (issue 422)

The reference is the permission prompt's own earlier behaviour and tests, in
`crates/extra/src/permission_prompt.rs`.

| What | Why | File |
| --- | --- | --- |
| A plain Enter no longer allows a risky or destructive prompt. Only the modified Enter (Command on a Mac, Control elsewhere) allows it. A plain Enter on the prompt does nothing, and Escape still denies. Safe prompts keep the plain Enter. The owner can turn this on or off with `deliberate_allow`. | The issue asks for a deliberate allow key, on by default for risky and destructive prompts. Risky is the default risk, so a prompt built with no risk set now needs the modified Enter. | `crates/extra/src/permission_prompt.rs` |
| With the deliberate allow key on, Enter or Space on a focused Allow button does nothing. A pointer click or the modified Enter still allows. | Without this, Tab to Allow and a plain Enter would still allow, and the check "plain Enter never allows" would fail. | `crates/extra/src/permission_prompt.rs` |
| The modified Enter now allows from anywhere inside the prompt, also when the deliberate key is off. Before, every modified key was ignored. | One key that always allows, the same as the approval card. A keyboard click never carries a modifier, so it cannot press a focused Deny button by mistake. | `crates/extra/src/permission_prompt.rs` |
| The default choices are once, this chat, timed and always. Before, they were once, timed and always. | The issue adds "this chat" as a fourth choice beside the other three. "For good" is still never offered for a destructive prompt. | `crates/extra/src/permission_prompt.rs` |
| The Allow button's key hint shows the modified Enter when the deliberate key is on. | The hint must name the key that works. | `crates/extra/src/permission_prompt.rs` |
| The choices of how long are a named radio group, each choice a radio button that says whether it is picked. The prompt's description names the picked answer and the keys. | The issue asks that the choices be announced. Before, they were plain clickable boxes with no role. | `crates/extra/src/permission_prompt.rs` |
