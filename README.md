# grm-vm

The GRM machine, in Rust: a network of nodes and relations that settles by itself (Gaussian belief propagation),
where every node and every relation is a small processor that only talks to its neighbours, and only works when a
neighbour has something new to say.

It is written to be fast, to never make anyone wait for it, to try new things apart before they touch what it
holds, and to talk to other machines without a centre. It has a prompt (`grm`), a window (`grm-app`), and a voice.
It is meant as a reactor: it starts with little, every way in and out within reach, and the human builds the
tools they want on it (apps), each one open, readable and changeable while it runs.
The Python machine of [grm-core](https://github.com/innermost47/grm-core) was its proof of concept; this one gives the same numbers.

Licence: AGPL-3.0-or-later. © 2026 Anthony Charretier.

## Build and run

```sh
cargo build --release
./target/release/grm               # a prompt
./target/release/grm program.grm   # says the file, then gives the prompt
./target/release/grm-app           # the window (same commands, same memory)
cargo test                         # everything, the parity with the Python machine included
```

On Linux, the sound needs ALSA's development files to build (`libasound2-dev` on Debian and Ubuntu); the window
needs nothing more than a graphics driver (OpenGL).

At the prompt: the language (GRML, as in grm-core: `node`, `relate`, `keep`, `observe`, `forget`, `var`, `set`,
`learn`, `input`, `output`, `fuse`, `network ... end`, `call`, `use`), and `help` for the rest.

```
grm> node x signed
grm> relate r: y = 2 * x
grm> observe x = 1 ± 0.001
grm> sandbox idea
grm> in idea: relate lie: y = 10 * x
grm> trial idea
  merge of idea: 0 new node(s), 1 new relation(s)
  harmony 1.000 -> 0.665
  surprise x: moved 1000.0 sigma
  surprising: it would need to be confirmed (not rejected)
grm> merge idea
grm> tick
grm> status
  ⚠ in quarantine: sandbox:idea
```

## What is in it

| | file | |
|---|---|---|
| the language | `lang.rs` | GRML, statement by statement, as grm-core reads it |
| the network | `network.rs` | nodes, relations, measurements; each with its **provenance** (who said it, from where) and its **strength** |
| the machine | `machine.rs` | belief propagation: rounds, ticks, `set`s, learning |
| the world | `world.rs` | the global network, **sandboxes**, **trial merges**, **integration little by little**, **quarantine**, **recall**, `remove`, the logbook |
| the engine | `engine.rs` | the machine in a thread of its own, computing a frame at a time, publishing snapshots |
| ports | `ports.rs` | what comes in and goes out: values, vectors, events (a click, a key), images and sounds (their adapters come with the senses) |
| clocks | `reflex.rs` | the speed of sound: a network's `set`s run directly, 48 000 times a second, tuned by the machine above |
| peers | `peers.rs` | machines that share what they believe, never their data, without a centre |
| hidden state | `hidden.rs` | what a node really is behind what is measured of it: a Kalman filter that follows it, an expectation-maximisation that consolidates its law, the world's noise told from the senses' |
| time | `time.rs` | nodes that **remember** (precise near the present, coarser further back), **learn how they move** in real time, **find links by themselves**, are **foreseen** with a confidence, and say **why** |
| the world outside | `grm-connect` | web APIs: what an answer can give (fields, kinds, units), the chosen fields into a sandbox, their rows each at its moment; Open-Meteo and Our World in Data to begin with |
| laws | `discover.rs` | what holds between nodes at the same moment (a line, a power, a product of powers), found in the rows it has seen and proposed through a sandbox |
| the eye | `vision.rs` | pictures: a network of its own near what it sees (pixels tied to their neighbours, a pyramid above them), that cleans, fills, keeps edges, is surprised, sees what changed, and sends a glance to the world's network |
| units | `units.rs` | a node with a dimension is given in SI, counted inside in Planck units |
| the prompt | `grm-shell` | every command, the same in the terminal and in the window: a line goes in, lines come out (said, dim, a warning, an error, a question waiting for y/n) |
| the sound | `grm-sound` | the machine's own voice, the mixer of the human's channels, the sound device, WAV files |
| the window | `grm-app` | the network, the prompt, the inspector, sandboxes, sources, journal, eye, sound; apps as code, network and surface; placed by a GRM network |
| apps | `grm-shell` (`apps.rs`, `runtime.rs`) | an app's file read (`use` and `call` unfolded, its surface and APIs set apart), run, edited while it runs, closed, kept with the desk |
| APIs | `grm-connect` (`api.rs`) | GET every so often, POST, WebSocket, each in its thread, what it reads into nodes |

### Never waiting, never computing for nothing

- **A round is shared out between threads** when there is enough to do, and gives the same numbers on 1 thread or
  32: each processor reads only what the round before wrote (tested bit for bit).
- **Only what moves is computed.** A processor that hears nothing new does nothing. A measurement wakes its node
  only. A network that grows (a relation added, a file said) goes on from where it was: only what is new is woken
  (adding one relation at the end of a chain of 300 costs a few messages, not the chain again).
- **Anytime.** The engine computes at most a frame (8 ms by default), then publishes where it has got to: what is
  shown is usable even before it has settled. A capped round (`Budget`) takes the biggest news first.
- **Nobody waits for the engine.** Whoever talks to it sends orders and reads snapshots.

### Trying apart, merging carefully: the beginnings of an immune system

- **A sandbox** is a network of its own. It reads the global network's beliefs at its border as fixed
  measurements, and computes only itself; the global network does not know it is there.
- **A trial merge** puts it into a working copy of the global network (starting from where the global machine is)
  and reports: new nodes and relations, what it would replace, names said differently, the relations that would
  hold less well, the harmony before and after, the nodes it would move further than 3 sigma. Something
  surprising is not rejected: it is said to need confirming.
- **Merging is gradual**: the new relations rise in strength (10 %, 25 %, 50 %, 75 %, 100 %), one step a tick.
- **Quarantine**: if the harmony falls (the relations, and what the world was measured to be) or a surprise
  rises while they rise, they go back to 0. They stay visible until released or recalled.
- **What a source taught can be taken back**: the weights keep what they learned by source; a quarantine or a
  recall takes it back.
- **Recall**: everything that came from a source, taken away at once, everywhere (what others still hold stays).

### Machines without a centre

Two machines that share nodes exchange messages about them (a precision and a mean): never their data. What a
machine tells a peer leaves out what it heard from it (nobody counted twice). What a peer says comes in as a node
of its own, tied by a relation of the peer's provenance, its certainty capped by the trust it has: a peer can be
put in quarantine or recalled like any source. The transport here is in-process; the internet's comes later,
with the same messages.

### Time: remembering, learning, foreseeing

- **Memory**: the last moments one by one, then spans twice as long, again and again (a memory that grows with
  the logarithm of time, each span with its mean and its spread; inspired by the scale-invariant memory of time,
  Shankar and Howard 2012). `rewind` reads it back.
- **A law of motion learned in real time**: its next value from its last ones (two lags: it can swing, with its
  period and its damping), from the nodes it is coupled to, and a constant; a Bayesian regression that forgets
  slowly, so it follows a world that changes.
- **Links found by itself**: shadow laws try the couplings a node does not have; one that predicts what comes
  clearly better (e^6 of evidence, measured before seeing each moment) becomes part of its law, and the logbook
  says so. A link that predicts is not yet a cause.
- **A hidden state**: once it has seen 64 moments, a node learns what it really is behind what is measured of
  it: the world moves (`x_t = a . x_past + b . u + c + w`), and is measured with an error (`y_t = x_t + v`). A
  Kalman filter follows it moment after moment (a missing measurement: it goes on, less sure); every 24 moments
  it consolidates its law, the world's noise and its senses' noise on the 480 moments it keeps one by one
  (expectation-maximisation with a Kalman smoother, Shumway and Stoffer 1982). A law that would blow up is
  brought back inside. On a swing read by a poor thermometer, its 24-moment forecasts are three times narrower
  than a law learned straight on the measurements, and still hold the truth.
- **Cycles found by themselves**: shadow laws also try the rhythms its past seems to hold (the strongest peak of
  its periodogram, and a day and a week when it counts in hours). A cycle is a rhythm it is measured with, on top
  of its state (an amplitude and a phase), learned with the rest.
- **Critical thinking**: a link or a cycle found quickly on the measurements is looked at again with the hidden
  state: it must explain what the node remembers clearly better than without it (by half the log of the moments
  per weight it adds, as the Bayesian information criterion asks); at once when it is found, and all its links
  every tenth consolidation. A link that does not pay for itself was an echo of the senses: the node stops
  believing in it, does not try it (or a rhythm close to it) again, and the logbook says so. On the weather, the
  temperature keeps its day; the humidity's day is dropped (it comes through the temperature it follows).
- **Searching afresh**: every tenth consolidation starts again from several points, not only from what it
  believed: a belief can be a dead end.
- **Thinking in parallel**: consolidating and thinking again are the long part; each node does it on its own,
  all at once, on as many cores as there are (the numbers do not depend on them).
- **Foresight**: the tracked nodes run forward together, 300 times, each run drawing a law it could have, where
  it is now, the world's noise and its senses' noise: the spread widens with the horizon.
- **Why**: a confidence (1 minus its uncertainty against how much it usually varies), where its uncertainty comes
  from (the world moving, its senses, its ignorance of its own law, not knowing exactly where it is now), the
  noises it has told apart, what each part of its law contributes, every node and source it
  rests on, and how many of its past forecasts fell within one sigma (does it know how much it knows?).

```
grm> connect meteo 48.86 2.35 30        # (or: connect file answer.json, connect owid co2 France)
grm> take meteo 4 5                     # the fields chosen become sandbox meteo
grm> trial meteo
grm> merge meteo
grm> feed meteo                         # 720 rows, each at its hour; found by itself: humidity follows temperature
grm> forecast temperature_2m 24
grm> why relative_humidity_2m 3
grm> rewind temperature_2m -240         # ten days ago, as remembered
```

### Memory between sessions

The whole world falls asleep in a file and wakes exactly as it was: its network, the state of every processor,
its sandboxes, what is rising and what is in quarantine, its logbook, its rows, its laws, and its nodes that
live in time (their memories, their laws, their hidden states). The prompt wakes from `grm.world` (or
`$GRM_MEMORY`) when it starts and falls asleep there when it quits; `sleep FILE`, `wake FILE`, `forget everything`.
A world put to sleep and woken goes on with the same numbers, to the last bit (tested).

The desk sleeps beside it: the apps that run and their own copies (`grm.desk.json`), the APIs said at the prompt,
what played and how, the machine's voice; and the window as it was left (`grm.window.json`): which app is shown
and how (code, network, app, launched, edited, x-rayed), the side panel's tab, the selection, where every point of
the network was put and how the view was moved, the panels' wishes, the log and the prompt's history. The world
is also put to sleep every minute while the window is open, and when it closes (Ctrl+Q).

### Laws at the same moment

The rows it measures are kept evenly along what it lived (400 at most: when there are too many, one in two is
let go). After a file, and on `discover`, it looks for what holds between its columns on every row (r² ≥ 0.999),
the simplest form first: `y = a x + b`; else the best of `y = k x^p`, `y = a ln(x) + b` (the Hubble law, in
magnitudes) and `y = k e^(a x)` (a decay); for what no single node explains, sums `y = a1 x1 + a2 x2 [+ a3 x3] + b`
and products of powers `y = k x1^p1 x2^p2 [x3^p3]` of two, then three nodes (the fewest that hold, each of them
counting); and two straight lines in two regimes of a third node (`when z in lo .. hi`, the value
where it changes searched among all). A law that needs a logarithm says the nodes that hold it. Coefficients are
written to twelve digits, and a law is never held surer than that (a law rounded and held exactly would
contradict the same law said the other way). Each law is written in its language (with its spread as its
sigma) and proposed through a sandbox: a compatible one is merged little by little; a surprising one waits in its
sandbox for a human.

### A value missing

A value missing from a row is no measurement. A node that lives in time is then given what it expected (its
forecast and its spread) as a soft measurement, so its relations still hear of it; it is never remembered as
measured.

### The machine following a node itself

`embody NODE`: the machine holds a node's hidden law as its own relations: its state, its states before (carried
from tick to tick together, with their uncertainty and how they move together: `carry x = y` and
`carry a, b = c, d`, new statements of the language), its law (the world's
noise as its sigma), and how it is measured (its rhythms, its senses' noise). The machine then follows the node
as a Kalman filter does, with every other relation of the network heard at once, and foresees the moments not
measured. On a world read by a thermometer erring by ± 0.5, what the machine believes is within ± 0.29 of the
truth. The law is held again each time it is consolidated.

### Believing together

`joint NAME...` (or a vector's name): what the network believes of several nodes together, as one Gaussian of
several dimensions: each value with its exact sigma, and how they move together. It is computed from the whole
network at once (its information matrix, drawn straight where it stands, solved by conjugate gradients): two
unknowns whose sum is known move against each other (-0.99), as they should.

### Beliefs held together

`carry a, b = c, d`: at each tick, what the network believes of c and d together (their means and their
covariance, computed from the whole network) becomes one prior over a and b: a state of several parts carried as
one. `observe v = [1, 2] cov [[1, 0.9], [0.9, 1]]`: a vector measured together; measuring one of its components
then moves the others with it. Such beliefs are factors of the machine over several nodes: belief propagation
carries what they say to each node.

### The eye

`see FILE.png [NAME]`: an eye sees a picture (grey or colour). It is a network of its own, as a retina is: every
pixel a node, tied to its neighbours ("I look like the one beside me, more or less"), and a pyramid above them
up to a glance of 8 x 8 nodes. It computes near what it sees and sends on only what is worth it:

- **cleaned**: its noise taken away, its missing pixels filled, its edges kept (a neighbour that differs much is
  believed less and less: robust weights, drawn again three times);
- **surprised**: at each pixel, how far what came is from what the rest of the eye said it would be, in sigmas
  (the message belief propagation already sends the pixel);
- **what changed** since the last picture, in sigmas;
- **a glance**: the top of its pyramid, heard by the world's network as inputs `NAME_glance_x_y`.

Pictures are written beside the one seen (`-clean`, `-surprise`, `-change`). An image port feeds an eye too.

![A square seen twice by the eye: what came, cleaned, surprised; then moved, cleaned, what changed](docs/eye.png)

On a 48 x 48 picture with noise of ± 0.10 and 15 % of its pixels missing, what the eye sees is within ± 0.028 of
the truth, the missing pixels within ± 0.04, and the edges stay sharp; a 64 x 64 picture takes half a second on
the machine it was written on, a 256 x 256 one about 7 seconds. Recognising what is in a picture is another step (features learned), not this one.

### The window

`grm-app` shows the machine as it is, and every button is a line said to the prompt (it shows in the log: nothing
happens behind the human's back).

![The window: a network, a relation that strains in amber, the inspector, the prompt asking before it removes](docs/window.png)

- **The network**: nodes (circles: filled when observed, hollow when the network computes them) and relations
  (diamonds; squares for limits), linked as the machine holds them. A colour per source (kept when others come or
  go); amber for a relation that strains (never red); faded for what is in quarantine; dashed for what rises little
  by little or sleeps. An eye's glance is one point. Click to inspect, Ctrl+click to add, Shift+drag a box, drag a
  node to put it somewhere, wheel to zoom, drag the background to move, right click for what can be done. It moves
  slowly, and stops when it is at rest.
- **The prompt**: history (↑ ↓), completion (Tab: commands, nodes, sources), Yes / No when it asks. In French,
  the machine's usual answers are said in French in the window (the others stay in English).
- **The inspector**: value ± sigma (a positive node's spread as a percentage), where it comes from, what it is
  (observed, computed, fixed, written by its sets, learned, an input, an output, in time), its relations with their
  strain; for a node in time, its memory and what it will be (a line, its spread, the forecast dashed with its sigma
  and the range of its draws), and why. Several nodes: what the network believes of them together.
- **Sandboxes** (say in it, run, trial, merge, refuse), **sources** (quarantine, release, recall), the **journal**,
  the **eye** (what it saw: cleaned, surprised, what changed), the **sound** (its voice, the channels).
- **Accessible and calm**: everything by the keyboard (Ctrl+L the prompt, Esc, F1 help, Ctrl+1 … 6 the tabs, Ctrl+.
  panic), screen readers through AccessKit, text size, dark or light theme, French or English; nothing flashes,
  what changes glides, and the window repaints only while something moves. (The machine itself still answers in
  English: its messages come from the engine.)
- **Placed by relations**: the panels are nodes of a small GRM network of their own. The window's size comes in as
  two inputs; soft wishes (`side = 0.3 * W ± 0.03`) and limits (`keep side >= 300`) decide; dragging an edge adds a
  wish, stronger than the share, still within the limits; a narrow window switches regime (`when W in 0 .. 860`)
  and the side panel goes under the network. The settings show its statements and its harmony.

![A narrow window: the side panel under the network; the eye's glance as one point](docs/window-narrow.png)

`grm-app --fresh --script LINES.txt --select a,b --tab sound --size 1280x800 --theme light --lang fr --screenshot
OUT.png` says the lines, takes a picture of the window and closes (to test it without anyone in front of it).
The drawing is behind the model (`model.rs`, `graph.rs`, `layout.rs` know nothing of egui): the renderer can
change without touching them.

### Apps: what the human builds on it

An app is one file of the language: its network, its surface, its APIs, the commands it runs when it opens.

```
app Oscillator bench
input pitch = 220 positive
input volume = 0.5 positive
input hold = 0
node step = 0.0288
relate turn_rate: step = 0.000130899693899575 * pitch
use networks/osc.grm                      # networks written once, used by many apps
call osc(name=lead, step=step)
node out signed
relate loudness: out = volume * lead_s + volume * hold * lead_s
widget p: slider pitch 55 .. 880 at 0 1 size 6 1 "pitch (Hz)"
widget go: play out at 0 2 size 3 1 "sound"
widget wave: scope out -1 .. 1 at 3 2 size 9 3 "the wave"
widget kb: keys pitch hold at 0 5 size 12 2
```

- **Three views of one thing**: its code (coloured, its line numbers, the lines it could not say in amber with
  why), its network (only what it holds and the nodes it touches), its surface. A change in one is at once in the
  others: there is no build. Each change is said again into the running app in one step (nothing in between is
  ever seen), it goes on from where it was (what its `set`s write, its variables, its inputs keep their values),
  what it plays goes on without a click, and the file is written at once. Undo, redo.
- **The surface**: widgets on a grid of 12 columns (the width is shared, whatever the window): `slider`, `toggle`,
  `button` (1 while held), `meter`, `value` (± sigma), `plot` (through time, with its spread), `text`, `picture`
  (what an eye saw), `pad` (two nodes at once), `keys` (a keyboard of notes: the frequency, and 1 while held; the
  computer's keys A W S E D F T G Y H U J K play it too), `play` (plays a node, stops it), `scope` (what a node
  that plays sounds like). A widget that touches measures its node; a widget that shows reads its value and spread.
- **Edit mode** (Ctrl+E, a button, also when launched): the app keeps running and sounding while its widgets are
  moved and sized (snapped to the grid; a widget never goes over another), chosen to change their line, added
  (kind, nodes, name), taken away. Made to be used live, on stage.
- **X-ray** (Ctrl+R): over each widget, its line in the file, the node it is bound to, its value ± sigma and where
  it comes from. Nothing of what an app does is hidden from who uses it.
- **Its network, by hand**: drag with the right button (or Alt) from a node to another: a relation, written first
  in a small window, then added to the app's file. A relation an app writes can be edited from the inspector
  (its line). `remove` of something an app writes asks, and takes the line from the app's file.
- **Launch** (F5): the app alone in the window. Esc: back.
- **Several networks, shared**: `network NAME(p, q) ... end` in a file, `use FILE`, `call NAME(p=..)`: unfolded
  into plain statements, so every name the machine holds is written somewhere. Two apps can use the same network:
  while only one runs, it holds it; when a second one opens while the first runs, the second runs its own copy
  (its names given its prefix, `synth__lead_s`), said in the log and in its bar.
- **Nothing forgotten by the machine**: what an app holds is never taken away by the machine itself; `remove`,
  `recall` and `quarantine` of it ask first. Closing an app (`app close NAME`, or its tab's menu) keeps its file
  and what it was living through, for when it opens again.
- **`clock S`**: the world ticks every S seconds while the app runs (the shortest asked wins).
- Apps are the `.grm` files of `apps/` beside the world's memory (or `$GRM_APPS`); `+` in the tabs opens one or
  makes a new one; `apps`, `app open|new|close|undo|redo NAME` at the prompt. Three come with it: an oscillator
  bench (sliders, keys, a scope, sound), the weather of Lyon (an API every ten minutes, a relation to Fahrenheit,
  a plot), a swing in time (a clock, a node tracked: its memory, its forecast, why).

![An app's surface: sliders, play, a scope of what it sounds, keys](docs/app-surface.png)
![The same app as code: what it said, coloured](docs/app-code.png)
![Edit mode: the app keeps sounding while its widgets move](docs/app-edit.png)

### Seeing it work

- **The master equation**, top left of the network: what the relations of products say together once every node
  in between is eliminated (in log, a chain of straight lines; what is left links the nodes at its ends), with how
  many decades its nodes span and how far it is from holding. Under it, the one of what is shown (an app, a network
  brought to the front). Nobody wrote it: the network holds it. `master [SOURCE]` at the prompt.
- **Under the pointer**, a node or a relation says everything: its value ± sigma, where it comes from, what it is,
  its relations or its equation, and its curve as it moves now (a node that plays: its wave, as it sounds).
- **Right click**: what can be done, each with its sign and a line saying what it does (inspect, follow in time,
  forecast, play, plot, relate to…, edit its line, quarantine its source, remove; on the background: a new node by a
  small form — its name, its kind, its value — written in the code of the app shown, a new relation, fit, plots).
- **The networks in the network**: a chip each above the view (`typed`, an app, a file, a sandbox), a soft halo
  of its colour around its nodes; a chip brings it to the front (`focus NAME`).
- **Plots** (desk or app): the nodes through time (one track each, never two scales on one axis), one against
  another, a path (a phase), their values now with their spread.
- **A sandbox shown** in the middle, as a network of its own (Sandboxes → Show), with run, trial, merge, refuse.
- **The code beside** (`</> code beside`, Ctrl+K): an app's code docked right, left, top or bottom while its network
  or its surface is shown; a line typed appears in the network, a node made by the mouse appears in the code.
- **The guided visit** (`tour`, or the help): one part of the window lit at a time, the rest dimmed; it comes by
  itself the first time.

![The master equation, the networks' chips, the halos](docs/master.png)
![Right click: what can be done, in words](docs/menu.png)
![The code beside the network, live](docs/code-beside.png)
![Plots: one track each](docs/plots.png)

### The machine's look

A palette (night, day, ocean, contrast, or each colour by hand in the settings: background, panels, buttons, text,
quiet text, accent, strain), a typewriter font everywhere if wished (on by default), the machine's name in block
letters when it wakes, a taskbar icon. Apps inherit it.

**Looks: style as a network, not as classes.** An app says `look NAME: accent = #7fb8f0, background = .., text =
.., round = 10, size = 1.3 [when NODE in LO .. HI]` and a widget wears it (`widget go: play out look big`). Each
property is a node of a small GRM network of its own: the machine's palette is measured into `vm_*`; the app's look
(`app_*`) is held to it, and each named look to the app's, by soft relations, each parent held a hundred times more
surely than its child (so the cascade goes one way); what a look says holds at ± 0.0001, so it wins; what it does
not say is inherited. A look with `when` holds only while its condition does; the hour comes in as an input, and
with "follow the hour" the evening warms the accent and dims the background (relations, not code). `style
STATEMENT` says anything of the language into that network (`style relate breathe: big_size = 1 + 0.2 * warmth`).

### Files, folders, data, and the words of the Python machine

The machine works in a folder, as a shell does, and the prompt says which (`grm ~/data›`): `pwd`, `cd [DIR | .. | ~ |
-]` (`~`, `$VARIABLES` understood), `ls [-l] [-a] [*.csv]` (folders, programs, tables each in their colour), `head`,
`tail [-n N]`, `cat`, `mkdir [-p]`, `cp [-r]`, `mv`, `rm [-r]` (asks first), `touch`, `new NAME.ext` (a file,
opened to write), `new NAME` (a program), `edit FILE`, `programs`, `run NAME`.

A file opened is a tab: text (a `.grm` coloured, run with ▶), or a table as a table (cells to change, rows and
columns to add, written back as it was separated). `load FILE.csv [COLUMN...]` gives its rows to the machine, one
a tick (columns never below zero are quantities); without columns, the window asks which ones. Any table: UTF-8 or
Windows', `,` `;` tab `|`, decimal commas, thousands set apart, percentages, a title line above the header; a header
that is not a name becomes one (`Température (°C)` → `Temperature_C`). A falling body's table, loaded: the machine
finds h = K t² by itself, and `ask` answers for a time it never saw.

`ask`, `put`, `why NODE` (what each relation tells it, each weighed; and, in time, its forecast), `inspect`,
`outputs`, `check`, `master`, `focus`, `play [HZ]` / `pause` (the clock), `history`, `!N`, `!!`, `clear`, `A ; B`,
`help WORD`, `tour`, `lang fr|en`, and the short forms (`nd`, `rel`, `obs`...).

**One world for everything, and its undo.** Every app, every file loaded, every sandbox merged lives in one global
network (each part with its provenance): that is what lets anything be linked to anything, shown, counted. `undo` /
`redo` give the whole world back as it was before the last change (30 kept); `stats` counts it by source, and what
the human says most (a first step for a machine that adapts to the one who uses it; see the roadmap).

### APIs

```
api weather: get https://api.open-meteo.com/v1/forecast?latitude=45.76&longitude=4.84&current=temperature_2m every 600
read weather current.temperature_2m -> temp ± 0.5
api hook: post https://example.org/in every 60 with temp humidity
api stream: websocket wss://example.org/feed send volume every 1
read stream data.price -> price ± 0.01
```

Each API lives in a thread of its own (an app's while the app runs, or the desk's when said at the prompt): GET
asked again every so often, POST sends the values of nodes as a JSON object, a WebSocket reads each message and
can send too (it reconnects by itself, waiting longer each time). What is read goes into its node as a
measurement with its sigma (a path: `a.b[0].c`, `[-1]` the last of a list, `.` the whole answer). What it says
(it connected, it could not reach the server) comes into the log.

### The sound

The human decides what is heard; the mixer only makes sure it never hurts.

- **Its voice** (`sound on`, `voice pad|engine|wind`): what the machine feels, heard. How much it computes moves
  its level and brightness; how well its relations hold chooses its chord (open when they hold, suspended when they
  strain); the relations that weigh most are its tones, a strained one detunes a little (a slow beating, never a
  clash); a drone under it, a soft bell when it finds a law or a link, a swell of air when it revises or
  quarantines, a room around it. It starts silent, everything glides over seconds, it never goes above 0.5.
  `cargo run --release -p grm-sound --example listen` writes 30 seconds of each voice into WAV files.
- **The channels** (`play NODE [GAIN]`): a node is played at the speed of sound. Its network's `set`s run 48 000
  times a second (a reflex), and every node a relation gives from them is computed at each sample too; the rest
  (the parameters) is tuned by the machine a few times a second, so a model of a string, an oscillator, a filter
  are heard while the human changes them. Gain, pan, mute, solo, stop (`gain NODE G`, `pan NODE P`, ...), a level
  meter per channel.
- **Safe**: every channel rises from silence and leaves into silence (30 ms), what is not a number is silence, a
  constant offset is removed, the whole never goes above 0.8 (a soft limit), `panic` (Ctrl+.) takes everything down
  in a breath. `render NODE SECONDS FILE.wav` writes a node into a file without a sound device.

```
grm> node step = 0.0576
grm> observe step = 0.0576 ± 0.000001
grm> var c = 1
grm> var s = 0
grm> node c_new signed
grm> relate turn: c_new = c - step * s
grm> set c = c_new
grm> set s = s + step * c_new
grm> play s 0.4          # 440 Hz, steady (symplectic Euler: its amplitude holds)
grm> observe step = 0.0864 ± 0.000001    # 660 Hz, at the next tuning
```

### Vectors

`node v[3] signed`, `var p[2] = 0`, `input pos[2]`: a vector, its components `v_0, v_1, v_2`. A statement that names
vectors holds for each component (`relate r: w = 2 * v + 1`, `set p = p + v`, `keep k: v >= 0`); `dot(v, w)` is a
sum of products; `observe v = [1, 2, 3]`; `track v` makes each component live in time.

## Parity with the Python machine

`tests/parity/cases.json` holds programs (products, sums, converters, `set` loops, learning, contacts, `ln`,
conditional relations, called networks, units, powers, a loop of sums). `tools/golden.py` runs them in the Python
machine of grm-core and writes `golden.json`; `cargo test` checks that the Rust machine gives the same values,
tick by tick, in the same number of rounds.

```sh
PYTHONPATH=path/to/pygrm/src python3 tools/golden.py path/to/grm-core/vm
```

## How fast

The same grid (`tools/grid.py`), the same 100 rounds, the same messages, on one core of the machine it was
written on:

| | nodes | processors | messages | Python (grm-core) | Rust |
|---|---|---|---|---|---|
| grid 40 x 40 | 1 600 | 4 720 | 1.39 M | 2.41 s | 0.11 s |
| grid 200 x 200 | 40 000 | 119 600 | 26.4 M | — | 2.3 s |

About 20 times faster on one core; more cores share each round (measured on the host it runs on: the machine it
was written on has 2 shared cores, too few to say).

```sh
cargo run --release --example bench -- FILE.grm [THREADS] [ROUNDS]
python3 tools/bench_python.py path/to/grm-core/vm FILE.grm [ROUNDS]
```

## What it does not do yet

- Not ported yet from the Python machine: `step` (one pass at a time), `watch` (breakpoints), `mood`, `capacity`,
  `prune`, the night (fusions, forgetting), the journal written for a human; `why` tells shares and what each
  relation says, not yet what each heard. A table opened shows its first 500 rows.
- The window is a first version: the desk's own panels (inspector, prompt...) are not yet apps the human edits;
  a surface's widgets are placed on a grid, not yet by relations of their own; a big network (more than about
  1 500 points) is drawn without the push between points.
- An app's state is kept for what its `set`s write, its variables and inputs and weights; a node's memory in time
  is kept while the node stays, not when its app closes.
- A surface reads the machine about 20 times a second, and the sound's parameters are tuned about 10 times a
  second: keys and sliders are heard within a tenth of a second, not at once.
- Inputs and outputs: values, vectors, events, pictures (the eye), sound out, files, HTTP and WebSocket. Not yet:
  the microphone (the ear), the webcam, MIDI, OSC, serial ports.
- The sound plays what a reflex can run (sums, products, logs solved for one node); a network that needs rounds to
  settle at each sample is not heard. Sources of video, a graph of sound and pictures wired like modules
  (TouchDesigner-like), plug-ins (VST) come with the apps.
- Belief propagation sends one number and its precision to each node: what links several nodes lives in the
  factors (relations, joint priors, memories of weights). Means are exact; each node's sigma is exact on a network
  without loops, and `joint` gives the exact joint belief of any nodes from the whole network.
- Laws at the same moment are looked for among up to three inputs (and two regimes of a third node).
- Links found in time are links across a moment (x before, y after); links at the same moment are what the
  relations of the network hold (the discovery of laws of the Python VM, not ported yet).
- Texts from an API are shown, not yet turned into nodes.
- The ear (sounds coming in) comes with the senses. A large picture is not yet seen in real time (a 256 x 256 one
  takes about 7 seconds on the machine it was written on: belief propagation crosses a grid one neighbour a
  round; solving the coarse levels first would cut it).
- Peers talk in-process: the internet transport, signatures, earned trust, debates, human experts and vaccines
  come with the decentralised network.
- A network solved from nothing still floods: a chain of 2 000 relations takes 2 000 rounds and 10 M messages
  (1 s). Growing a settled network does not.

## Roadmap

See [ROADMAP.md](ROADMAP.md).
