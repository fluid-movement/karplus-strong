# Listening

`scenarios/` proves correctness. This is taste. No amount of green makes
listening optional.

```
just render
```

renders every file here, and every scenario, to `listening/out/` as WAV.
The files are in the scenario format, and they carry assertions, but only
the ones that would mean something is broken; nothing here is trying to
prove a target. If you find yourself tightening one, it probably wants to be
a scenario instead.
