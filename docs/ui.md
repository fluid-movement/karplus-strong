# UI

Read when editing the GUI layout or controls.

`KarplusStrongEditor` — `src/PluginEditor.h` extends `AudioProcessorEditor`.
Reference to the processor is held as `processorRef` for APVTS attachments
only — the editor never touches the synth or DSP directly.

## Data-driven controls

All controls are built by `addControl(group, paramId, labelText)`
(`src/PluginEditor.cpp:39`) and stored in a single
`std::vector<std::unique_ptr<Control>>` (`src/PluginEditor.h:21`):

- The control **type is derived from the parameter**: an
  `AudioParameterChoice` gets a `ComboBox` whose items come straight from the
  parameter's `choices` (so adding a choice in `PluginProcessor.cpp` updates
  the UI automatically); everything else gets a rotary `Slider`.
- Each `Control` owns its label, its slider-or-combo, and its attachment.
  The attachment members are declared after the component members, so they
  destruct first — the required order.
- Layout order **is** declaration order: `layoutGroup` places a group's
  controls left-to-right in the order of the `addControl` calls in the
  constructor, wrapping to a new row whenever the next control would overflow
  the group's inner width.

## To add a control

One line in the constructor:

```cpp
addControl (Group::delayLine, "my_param", "My Param");
```

`layoutGroup` wraps automatically to a new row once a group's controls no
longer fit on one line, so adding controls doesn't require manually
rebalancing column counts — it only needs enough window height for the extra
row(s). If a group grows tall enough to clip, increase `setSize` (`:36`).

## Window & layout

- Size: **800 × 420** — `src/PluginEditor.cpp:36`.
- Background: `0xff1a1a2e`.
- Title: "Karplus-Strong", centered, 18pt bold, top 30 px.
- Two `GroupComponent` panels: **Exciter** (left, width 320, column width 70)
  and **Delay Line** (right, remaining width, column width 58). Each wraps
  into multiple rows now that they hold more than one row's worth of
  controls.
- Geometry constants in `layoutGroup`: `lblH` 16, `ctrlH` 84, `ctrlGap` 4,
  `rowGap` 4.

## Current controls

| Group | Param id | Label | Control |
|---|---|---|---|
| Exciter | `excitation` | Excitation | ComboBox (from choices) |
| Exciter | `excitation_length` | Exc Length | rotary |
| Exciter | `pick_position` | Pick Pos | rotary |
| Exciter | `pick_model` | Pick Model | ComboBox (from choices) |
| Exciter | `sine_harmonic` | Sine Harm | rotary |
| Exciter | `exciter_tone` | Exc Tone | rotary |
| Exciter | `vel_excitation_length` | Vel->Length | rotary |
| Delay Line | `decay_time` | Decay Time | rotary |
| Delay Line | `brightness` | Brightness | rotary |
| Delay Line | `vel_brightness` | Vel->Bright | rotary |
| Delay Line | `vel_decay` | Vel->Decay | rotary |
| Delay Line | `output_level` | Output | rotary |
| Delay Line | `voices` | Voices | rotary |
| Delay Line | `key_track` | Key Track | rotary |
| Delay Line | `drive` | Drive | rotary |
| Delay Line | `damp_mode` | Damp Mode | ComboBox (from choices) |
| Delay Line | `release_time` | Release | rotary |
| Delay Line | `humanize` | Humanize | rotary |
| Delay Line | `stereo_spread` | Stereo | rotary |

## Styling

- Sliders: `RotaryHorizontalVerticalDrag`, `TextBoxBelow` 70×16.
- Labels: 12pt bold, centered, `0xffcccccc` text.
- Group outlines: `0xff444466`; group text: `0xffaaaaee` (`setupGroup`, `:12`).
- Fonts use `juce::FontOptions` (JUCE 8 API — the bare `Font(float, int)`
  constructor is deprecated).
