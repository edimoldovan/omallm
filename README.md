# omallm

A chat window for Omarchy. Ask how to do something in your own words; a small language model trained only on Omarchy's own text answers with the command, the keybinding, or a paragraph from the manual, and the app runs the command after you look at it and click Run.

```
› how do I update omarchy
  omarchy-update [-y]            Update Omarchy and system packages · needs sudo, runs in a terminal
  [Run] [Copy] [In terminal]
```

The model is the one from [slm-training](https://github.com/edimoldovan/slm-training) (29.5M parameters, from scratch, no outside text). llama.cpp is embedded in the binary; nothing else has to run.

## Run

```
cargo build --release
./target/release/omallm
```

The model file is read from the first argument, else `$OMALLM_MODEL`, else `~/models/omallm.gguf`. Get `run-006-Q8_0.gguf` from the training repo's `gguf/` folder and point one of those at it.

`omallm --ask "How do I take a screenshot?"` prints the answer and exits without running anything.

## What it does

- A command answer is shown in an editable field with the script's one-line summary. `[...]` optional parts are dropped; `<...>` parts you fill in before Run accepts it.
- Run executes only a file that exists in `~/.local/share/omarchy/bin`, without a shell; output streams into the chat with the exit code. Scripts marked `requires-sudo` open in a terminal window instead. Stop kills a running command; In terminal moves an interactive one to a terminal.
- A keybinding answer (`SUPER + RETURN`) and a manual paragraph are shown as text, with no Run button.
- Ctrl+K opens the command palette; Ctrl+Enter runs the newest command.

Built with [omarchy-kit](https://github.com/edimoldovan/omarchy-kit), so it follows the Omarchy theme live. Needs cmake and clang to compile llama.cpp through the `llama-cpp-2` crate.

See `PLAN.md` for what is built and what is not.
