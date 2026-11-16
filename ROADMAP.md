# Roadmap

1. **The heart** (0.1.0): the machine, the language, threads, frames, sandboxes and merges, quarantine, ports,
   clocks, peers in-process, parity with the Python machine. *Done.*
2. **Temporal nodes** (must have): memory precise near the present and coarser further back, rewind; laws of
   motion learned in real time (oscillators coupled to each other), links found by themselves; forecasts with a
   confidence and their reasons; the weather (Open-Meteo) and Our World in Data as first sources. *Done, first
   version.* A hidden state tells the world from the senses; links it stops believing in. Cycles found by themselves, and
   dropped when they do not pay for themselves. Sleep and wake, laws at the same moment, missing values foreseen,
   thinking in parallel, vectors. The machine follows a node with its own law
   (`embody`, `carry`), joint beliefs (`joint`), states carried together (`carry a, b = c, d`), vectors measured together, laws
   with logarithms, exponentials, sums and products of up to three nodes, and regimes.
3. **An interface made of nodes and relations**: elements as networks, layout as relations with their softness,
   responsive by tension, events as inputs; editable by the keyboard (the language, the terminal) and the mouse
   (right click, drag to relate); accessibility (screen readers) from the start; calm (nothing flashes).
   *First version done*: the window (egui) with the network, the prompt, the inspector with time and why,
   sandboxes, sources and quarantine, journal, eye, sound; layout by a GRM network (window size as inputs, soft
   wishes, limits, a regime for narrow windows); keyboard, AccessKit, text size, themes, French and English. The
   machine's voice and a mixer of the human's channels (gain, pan, mute, solo, fades, DC, limit, panic).
   *Apps done*: an app is a file (network, surface, APIs); three views of it (code, network, surface), edited
   while it runs from any of them (said again at once, written at once, undo), edit mode and x-ray, launch;
   networks shared between apps (a copy only when two run at once); drawing relations by hand; the desk kept as it
   was left; APIs (GET, POST, WebSocket); the machine's answers in French. Next: the desk's own panels as apps,
   surfaces laid out by relations, the ear, webcam, MIDI, OSC, serial ports.
   *Then done*: the machine's look (palettes, colours by hand, a typewriter font, the banner, an icon); looks as a
   network (inheritance by soft relations, conditions, the hour); the master equation; rich hover with moving
   curves; right-click menus in words; making nodes by a form; plots; sandboxes shown; the code docked beside;
   files, folders and tables (a table editor, columns chosen); `ask`, `put`, `why`, `check`, `stats`, `undo` of the
   whole world; a guided visit. Next: `step`/`watch`, the night (fusions, forgetting), an organism that adapts to
   the one who uses it (the global network watching how it is used: what is touched, when, together).
   *Then done*: back and forward; one undo for every change; files dropped or chosen; the inspector as an
   accordion; a coloured prompt; Lucide signs everywhere (and on apps).
   **Agreed next, in this order** (with Anthony):
   1. *A space, not a desk*: no tabs; an infinite table where the human puts what they want, in their own mess,
      with a button that tidies it (relations only when asked). An app and a widget are the same thing: small on
      the table it is a widget, bigger a window, or the whole screen. The space is itself a `.grm` file, coded and
      changed while it runs; several views of it (free table, grid, list, one thing); several spaces. The clock and
      the weather become ordinary apps.
   2. *Sound*: nodes that send impulses to others (a `pulse`, relations that act on it, a clock inside the sound
      thread so that pulses fall on the sample); sequencers made of it; a player of sound files (WAV, then MP3,
      FLAC, OGG by a pure-Rust decoder) whose features come in as nodes, and a screen drawn by the network from
      them, calm.
   3. *Apps of its own, written in GRM*: a spreadsheet (a cell a node with its spread, a formula a relation;
      the result fixed, the input found); a text editor; an SVG maker with nodes (shapes as nodes, geometry as
      relations), and a free paint that keeps what is drawn in the GRM.
   4. *Questions to data*: for any analyst, in the plots, buttons with questions ready-made from the data given
      (what next, what if, what drives what, how sure), then the prompt; forecasts drawn with their bands, and an
      honest confidence (the machine tests its bands on data it has not seen and says how often they held).
      Later, talking to the GRM in natural language.
4. **The senses**: images (a pyramid of patches), sounds (bands of frequency), seeing the network pulse, dreaming.
5. **The decentralised network**: the internet transport (no central server), public, private and closed networks,
   signatures, the sas (identity, form, permissions, quarantine, compatibility, earned trust), debates between
   hypotheses, human experts, vaccines.
6. **Apps made of networks**: a word processor, a spreadsheet, a DAW, a physical model of a guitar shared with
   others; programming by networks; flows wired like modules (sound, pictures, video, as in TouchDesigner) on the
   mixer's sources; plug-ins (VST) loading GRM packages.
7. **Further**: a bootable USB key, GRM-OS.
