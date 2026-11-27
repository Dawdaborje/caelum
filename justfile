# Run a binary in nested KWin with one output: just nested foot
nested +cmd:
    tools/nested.sh {{cmd}}

# Same with two outputs: just nested2 foot
nested2 +cmd:
    tools/nested.sh --outputs 2 {{cmd}}

# Print a docs/memory.md row for a binary (two outputs, 60s idle): just mem foot
mem +cmd:
    tools/memcheck.sh {{cmd}}
