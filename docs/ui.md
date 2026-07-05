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
- Layout order **is** declaration order: `layoutGroup` (`:93`) places a
  group's controls left-to-right in the order of the `addControl` calls in
  the constructor (`:23-34`).

## To add a control

One line in the constructor:

```cpp
addControl (Group::delayLine, "my_param", "My Param");
```

If the panel overflows, reduce that group's column width in `resized()`
(`:89-90`) or widen the window (`:36`). Formula:
`N * (columnWidth + ctrlGap) - ctrlGap` must fit in `panelWidth - 24`
(the `reduced(12, 20)` padding).

## Window & layout

- Size: **800 × 340** — `src/PluginEditor.cpp:36`.
- Background: `0xff1a1a2e` — `:71`.
- Title: "Karplus-Strong", centered, 18pt bold, top 30 px.
- Two `GroupComponent` panels: **Exciter** (left, width 320, column width 70)
  and **Delay Line** (right, remaining width, column width 58).
- Geometry constants in `layoutGroup` (`:95-97`): `lblH` 16, `ctrlH` 84,
  `ctrlGap` 4.

## Current controls

| Group | Param id | Label | Control |
|---|---|---|---|
| Exciter | `excitation` | Excitation | ComboBox (from choices) |
| Exciter | `excitation_length` | Exc Length | rotary |
| Exciter | `pick_position` | Pick Pos | rotary |
| Exciter | `pick_model` | Pick Model | ComboBox (from choices) |
| Delay Line | `decay_time` | Decay Time | rotary |
| Delay Line | `brightness` | Brightness | rotary |
| Delay Line | `vel_brightness` | Vel->Bright | rotary |
| Delay Line | `vel_decay` | Vel->Decay | rotary |
| Delay Line | `output_level` | Output | rotary |
| Delay Line | `voices` | Voices | rotary |
| Delay Line | `key_track` | Key Track | rotary |

## Styling

- Sliders: `RotaryHorizontalVerticalDrag`, `TextBoxBelow` 70×16.
- Labels: 12pt bold, centered, `0xffcccccc` text.
- Group outlines: `0xff444466`; group text: `0xffaaaaee` (`setupGroup`, `:12`).
- Fonts use `juce::FontOptions` (JUCE 8 API — the bare `Font(float, int)`
  constructor is deprecated).
