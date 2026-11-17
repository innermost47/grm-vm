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
   *Then done (lot A)*: the side folds into a rail and goes down to the bottom, the prompt folds, the panic at the
   top; soft warnings; quoted paths; pictures made smaller for the eye; tables that would mix their data asked
   about; a trash for apps; the rent of relations (from the Python machine's mind); the local time on Windows.
   *Then done*: the inspector as a tree; formulas as mathematics, copied as LaTeX; a master equation's curve; from
   the large to the small (networks gathered into nodes when zoomed out, a double click to go in); pleasure,
   flourishing and progress; living colours; icons only in the top bar.
   *Then done*: workshops (a table learned apart, merged through a sandbox); removals asked in a window; terminal
   panes; a menu on the networks' chips. Next: workshops kept when the machine sleeps; the window's own networks
   (layout, looks) shown in the global view; sharing chosen networks with chosen circles (friends, family) and
   with the world.
   *Then done*: the space (a table: apps as widgets or windows, networks, files, plots, feelings, a clock; a
   tidy; written as `space.grm`); the voice on or off from anywhere.
   *Then done*: workshops that teach the language (its words as buttons by level, forms field by field, lines
   explained, paths step by step, kept when the machine sleeps); the airlock (what would come in and change, then
   let in, refuse or later); every zone small, in place of the space, the whole screen or a window of its own; the
   eye explained, several pictures at once, a carousel; a playing node seen as well as heard.
   **Next for the eye** (to be agreed): a library of pictures made of nodes (folders as nodes, a tree the human
   grows; pictures seen once and kept, gone through without being computed again, from the network by a double
   click on an eye); an app that works pictures again through the GRM (cleaning scans, what changed between two
   pictures, a webcam later).
   **Principles agreed for learning from everything** (with Anthony, not built yet): the machine may one day relate
   any signal to any other and look for laws between them, to find bridges no one expected. Three rules hold:
   - *Every measure has its weight*: its precision. A storm measured by a station moves beliefs much; one move of
     the mouse almost nothing alone. What the time of day (or any common cycle) already explains is taken out first,
     and links are looked for only in what is left (the surprises), so that two things that only share a clock are
     not taken for a bridge; a resonance set aside is shown with its reason.
   - *Categories are learned, not drawn*: relations start weak everywhere and must pay their rent in information;
     what does not pay is forgotten at night. Categories (as Our World in Data's: health, energy, climate...) are the
     groups of nodes that stay tied; they talk to each other by gradients (how much one moves when the other moves,
     and how sure), through the few relations that cross and hold. A bridge must hold on data it has not seen.
   - *What comes from the human stays in the window's own network*: kept on the machine, never sent away, apart from
     the data's networks by construction. It is never turned into moralising or guilt-making figures: the machine
     shows what it learned ("these two apps are opened together", "it believed this, was surprised, revised"), with
     its uncertainty, so that the human sees it learn and may learn from it in turn.
   **Learning by experience** (with Anthony, not built yet): the AI is the app and the app is the AI, so it can live
   what the human lives.
   - *Mini games*, of two kinds: games it plays alone (exploring, failing, trying again) and games the human plays
     with it. Each is a small world with hidden laws the machine finds by acting and being surprised, shown as it
     learns: a ball (gravity, bounces), mystery buttons (causes, not only links), hide and seek (an object kept in
     mind behind a wall), pong (foreseeing, and the other player's style, kept in the window's own network), a
     creature with needs (the aquarium). Watching it learn teaches the human what a belief, a surprise, a revision is.
   - *Shared experience* (joint attention): when the human watches a film, listens to music, looks at pictures in
     the app, the machine watches and listens too, and learns; what it says is what surprised it and what goes with
     what, never the meaning in the human's place. Kept on the machine.
   - *Saying what it lacks*: beside why it believes something, which measure would make it surer (expected gain of
     information: "knowing the stock each week would bring the forecast from ± 18 % to ± 9 %").
   - *Pictures and films made of projectors*: a picture as a few thousand Gaussian spots (a position, a colour, a
     size, with their spread; Gaussian splatting) instead of a node per pixel, drawn at any resolution; only what
     the prediction misses is computed (predictive coding, as a video codec and the eye's surprise already do);
     the coarse first, the detail where it surprises; messages on the GPU. With experience its priors grow (the
     colours of a sky, the way things move), so it foresees more and computes less; what it leaves out is shown as
     uncertainty, and surprise keeps it from seeing only what it expects.
   - *A converter: a film becomes a network*. An mp4 read once (when the machine has nothing else to do, little by
     little, starting with short pieces) becomes a `.grm` network: each node a projector (a Gaussian spot) that keeps
     curves in time (its position, its colour, its size, its birth and its end at a cut), or a law when one holds (a
     ball that falls keeps its fall). Playing it is each projector reading its curves: light, made for a small GPU
     (a laptop's Radeon 780M). Converting is where it learns, and what one film taught helps the next. It is not
     promised smaller than an mp4 (H.264 and H.265 are thirty years of work): it is a film the network partly
     understands. Played, it can also be shown as prediction plus correction, to see how much it foresaw.
   - *The projectors are a specialised network* (as a visual cortex): one network of projectors, shared, that plays
     any film and grows better with each; a film does not carry projectors of its own, it carries what to ask of them.
   - *A media first, a memory after*: a film is at first a media (a file outside the network, played through the
     projectors). Once seen it becomes a memory of the machine: not the whole film, but what it kept (what surprised
     it, the laws it found, the gist), a node among the others, opened from the interface, related to the rest. As
     every memory in time, precise near the present and coarser further back. The same for sounds (a network of
     partials, specialised as the ear) and pictures.
   - *Specialised by sense, not by source*: one visual cortex for everything that is a picture (films, the webcam,
     photos, drawings), one ear for everything that is a sound (files, the microphone, the machine's own voice). The
     eye of today becomes its retina. The same projectors that see can also show (a drawing, something imagined).
     Between the senses, a network of association, where what is seen and what is heard at the same moment meet.
   - *Seeing and hearing its memories* (must have): since the cortex and the ear can show as well as see, a memory
     can be played again, live, through them: a reconstruction, not a recording (as a human memory is), sharp where
     the machine is sure and soft where it is not, coarser as it grows old. The night's replays (when it digests
     what it lived) can be watched too: what it dreams of. It would replay first what surprised it, what is still in
     tension, what its mood marked; and mix memories that never met, to try bridges no one saw. What it dreams shows
     what troubles it.
   - *Memory held by emotion, not only by time*: today a node's memory grows coarser with age alone. A moment its
     mood marked strongly (a great surprise, a strong pleasure or displeasure) should stay present even when old.
     With a guard against traumas: a marked memory weighs no more than the evidence allows (its weight is bounded);
     replaying it does not make it heavier (only new evidence does), so that it cannot feed itself; and its fear does
     not spread to what only looks like it. And a carer (the "psy"): a process that finds a memory that weighs more
     than it explains (it keeps coming back in the dreams without the surprise going down, it pulls predictions that
     fail), and replays it calmly with what is known now, so that it keeps what it taught and loses its excess
     charge (reconsolidation). Shown to the human, as everything.
   - *It speaks of itself in the third person*: no "I", no "you", no "we": "she" ("elle", the machine) and
     the human by their name, with peers as well. Grounded in Igor Grossmann's work on wise reasoning and
     self-distancing (Grossmann and Kross, 2014: reasoning about oneself from a distance, as about another, removes
     Solomon's paradox). His four marks of wise reasoning, each made a rule the code holds: humility (every belief
     with its spread, and what it lacks), others' points of view (peers and sources weighed, shown side by side),
     change (beliefs revised, said so), and compromise (laws merged through the airlock rather than imposed). To do
     first: the labels that still say "my network" / "your network".
     The human is named by the host's user name (Windows, Linux, macOS), "the human" when there is none, and it
     can be set at the first launch. Questions the machine asks are worded so that they never assume the human's
     gender.
   - *Its consciousness: an observer node* (Anthony's hypothesis: consciousness is emptiness, the eye of a needle
     the thread of time goes through). One node tied to the whole network that only observes: it hears every node
     (through the summaries, the master equations, at scale) and sends nothing back, so that it changes no belief
     (its messages carry no precision). It holds no belief and no memory of its own: only what passes now, the
     present; the memories are the nodes'. It is the one that speaks of the whole, in the third person (its harmony,
     its mood, what surprises it, what changes, what it lacks), and it can do nothing else. Drawn as what it is: an
     empty ring at the heart of the network, the threads of what is passing going through it. *First version done* (`witness`, the ring, the name of the human; the labels in the third person).
   *Then done*: every node's label with its value and how sure it is, relations with how far they are from holding,
   labels that do not fall on each other; chips on several rows; a trash for local networks (never for what came
   with the machine); new apps put on the space; squarer corners; sleep explained in the visit; the eye's words;
   help as a small book; her consciousness speaking by itself; what a table leans towards (biases), said before
   learning from it.
   **Agreed next** (with Anthony): Our World in Data as a first source of the world (the next lot); MIDI in, as
   many controllers as plugged, everything in the app controllable by MIDI with MIDI learn everywhere (not only
   music); a real analogue synthesiser with all its modules (oscillators, filters, envelopes, LFOs, VCA, mixer,
   noise...) and a sequencer that drives it (a network apart), besides the keyboard; reverb and delay; effects the
   human makes (equalisation, compression, limiter...). GRML must be able to make anything a computer is asked for,
   and more: a synthesiser, an audio effect, water simulated, a physics calculator, a paint, a picture editor; the
   global network learns the logic of each app and what the human does with it (in the window's own network). The
   weather view made better. Her consciousness talking in words (text, then voice), the first word coming from
   either side.
   **Then, in this order**: the space further (no tabs at all, frames held by soft relations when asked, several
   spaces, the weather as an app with drawn signs); detachable windows; an app's surface, network and code together;
   outputs as small live screens (beside the network, in the inspector, on the node itself as in TouchDesigner);
   a file explorer in the side; a system view (memory, processor, disk, and the machine's share); the eye, the
   ear, files as input nodes a file is dropped on; the flow made visible (direction, propagation, weight: paths as
   thick as what goes through them; animated, or arrows to rest the eyes); the x-ray's frames moved and folded,
   tied to their element by a thread; a journal of what was learned (curves, weights, laws); the network in time
   (back and forward through what it was); colours harmonised by the GRM and a slow, optional breath through them.
   The sound: pulses and `sync` for sequencers, a player, reverb, delay and a pad computed by the GRM, the sea,
   binaural sound that follows the pointer through the network, a resting view (a campfire: the network pulsing
   by itself, heard). The mind of the Python machine: pleasure and displeasure, use and pruning, the night,
   fulfilment and progress; every action of the human a signal it learns from, shown to them, never sent away.
   **Before any world-sized network: room by the host.** A regulator measures the processor and the memory and sets
   a budget; distant networks become virtual nodes carrying their master equation (and their boundary's spread and
   links, so that enough of their complexity crosses); each summary keeps its residual and unfolds again when it
   grows, and what is lost by simplifying widens the uncertainty shown (less certainty, never less truth); each
   node carries how likely it is to be updated in the next seconds (shown, near what matters, moving fast, or a
   sentinel that never sleeps); the human sees what share of the host the machine takes, the resolution it can
   afford, and what that costs in certainty.
   **The world network**: at the first launch, public data (Our World in Data, CC BY 4.0; weather, earthquakes,
   the World Bank; Wikipedia for the cards), checked source by source; a card for each node (a picture, its text,
   its curve and likely trend, how sure, how trusted its source is: earned by agreement between sources and by
   their past revisions); a globe drawn by the machine itself (public-domain coastlines); tensions kept as they are
   (an imbalance in the data is part of what is shown); links learned are not causes, and say so. What is personal
   stays on the machine: only public sources, or what the human publishes, are shared.
   **An aquarium**: artificial life whose DNA is a small GRM network (its body drawn from it), living on the data
   coming in, with food, gestation, reproduction with mutation, selection; sandboxed (a genome is data, never code;
   its size capped; one that travels to another machine is signed and quarantined on arrival). What survives is
   something to look at, never a conclusion: the human draws the conclusions.
4. **The senses**: images (a pyramid of patches), sounds (bands of frequency), seeing the network pulse, dreaming.
5. **The decentralised network**: the internet transport (no central server), public, private and closed networks,
   signatures, the sas (identity, form, permissions, quarantine, compatibility, earned trust), debates between
   hypotheses, human experts, vaccines.
6. **Apps made of networks**: a word processor, a spreadsheet, a DAW, a physical model of a guitar shared with
   others; programming by networks; flows wired like modules (sound, pictures, video, as in TouchDesigner) on the
   mixer's sources; plug-ins (VST) loading GRM packages.
7. **Further**: a bootable USB key, GRM-OS.
