# omallm: ask Omarchy, run the answer

A chat window for Omarchy. You type a question in your own words, the small model trained on Omarchy's own text (`run-004` from `slm-training` on the Mac) answers with the command, and the app runs it after you look at it and click Run. llama.cpp is embedded through the `llama-cpp-2` crate; no server.

```
question ──> embedded model ──> omarchy-theme-set <theme-name> ──> is it a script in ~/.local/share/omarchy/bin?
                                                                      │ yes: command shown in an editable field, [...] parts dropped
                                                                      │      <...> parts: you fill them in before Run
                                                                      ├─ keybinding answer (SUPER + …): shown only
                                                                      └─ anything else: "not an Omarchy command"
Run ──> plain script: runs without a shell, output streams into the chat, exit code at the end
    └─> script marked requires-sudo: opens in a terminal window (omarchy-launch-terminal)
```

## What is built

- `src/model.rs`: loads the GGUF once, greedy decoding, 64 tokens at most, stops at `<|end|>`. The prompt is `<|q|>question<|a|>` with special tokens parsed and bos added, the same ids as the training tokenizer.
- `src/omarchy.rs`: reads every `omarchy-*` script's `summary` and `requires-sudo` header lines; classifies an answer as command, keybinding or unknown; drops `[...]` optional parts; keeps `<...>` placeholders for you to fill.
- `src/runner.rs`: runs a confirmed command with `std::process::Command` (no shell), streams stdout and stderr, Stop kills it; sudo scripts go to a terminal window that stays open until Enter.
- `src/main.rs`: the window with omarchy-kit. A `TextField` (hint "Ask Omarchy", key `/`), Enter or Ask sends. Each turn: the question, the command in an editable field, the script's summary, Run (primary on the newest), Copy, In terminal, Stop while running, the output block. `StatusLine` with the model file name. `CommandPalette` on Ctrl+K: Run, Copy command, Open in terminal, Stop, Clear chat. Ctrl+Enter runs the newest command.
- `omallm --ask "question"` prints the model's answer and exits, nothing is run.

Model file: the first argument, else `$OMALLM_MODEL`, else `~/models/run-004-Q8_0.gguf`.

Build and run:

```
cargo build --release
./target/release/omallm
```

Dependencies: `omarchy-kit` (path `../omarchy-kit`), `eframe`/`egui` 0.33.3, `llama-cpp-2` 0.1.160 (compiles llama.cpp, needs cmake and clang), `shell-words`.

## Not built

- Arguments filled in by the model from the question ("set the theme to Tokyo Night"). Needs retraining with pairs that carry concrete values; see the training repo's `runs.md` for the model so far. Until then placeholders are filled by editing the command.
- Tests, packaging, a settings dialog.
