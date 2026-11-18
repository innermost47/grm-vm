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
      them, calm. *Player done* (MP3, FLAC, OGG, WAV, M4A by Symphonia, lists, a mixed order without
      chance), heard by her through the room; *her reverie done* (three veils of colour from her state, lit by what
      she hears and sees). Next: video for her eye, her ear telling her own voice from the room's. *The sound keeps up* (computed ahead, a load meter,
      what is slow computed slowly; *steady in silence*: no subnormal numbers, the sound's lock held for a moment,
      only what moved told to it, her ear resting when nothing sounds; *shared among the cores*: her, the instruments
      that follow her, what mixes them; what a knob alone moves computed when it moves; *the view light*: prepared
      once a shape, its layout resting, packets from afar). Next for the speed, if needed: her own network split
      (she is now the heaviest part), and her formulas compiled rather than read. *The universe of the GRM done*
      (grm-core's network in the machine, in three dimensions, its equations found by itself). Next: the
      simulation of the universe's eras, and her discoveries said back into the network as relations to weigh.
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
   *Then done*: Our World in Data (twelve indicators in six categories, a workshop each, kept a week); `delay`, and
   an analogue synthesiser all in networks (saw, square, four-pole filter with resonance, envelope, amplifier, echo,
   reverb as a feedback delay network, a sequencer of eight steps apart); MIDI in from every controller, MIDI learn
   on sliders, nodes, keyboards and lines; `why` and the trash said in French; her consciousness tied to all,
   flowing only towards it; the sound device fading instead of clicking when the mixer is held.
   **Still to do from the list below**: equalisation, compression, a limiter written as networks (the brick is there);
   MIDI learn on every control of the window (tabs, buttons), noise for the synthesiser, the weather view.
   *Then done*: a node in hand (slider and field; locked when it comes from outside), simulations (a copy apart,
   before -> after, written to share), the Insights tab (reality, the reflection with strains and levers, what is
   shared); knobs, faders and step lights; small frames that say less instead of spilling; what would help her
   feel better, in signs; the journal as a terminal, its changes as buttons; the prompt's lines selectable; a
   double click on a gathered network comes close to it and dims the others; Our World in Data's new headers.
   *Then done*: her consciousness as a triangle, its lines with signs, its equations the Planck units; a desk look
   for apps (sections, knobs with scales, switches with lights, steps with note names); two full envelopes; the
   trash asked softly and toasted; a sandbox let in shown as such; improvising by ear (the next notes written from
   how clear she hears herself; the learning in the window for now, to move into nodes).
   *Then done*: by ear, the step read from the sound itself (it was read from the machine, too slow: she heard but did
   not learn); knobs tried and kept or turned back; learning from what the human plays; a melody over the bass, what
   sounds together learned, the bass played by her with the sequencer off, the sequencer turned on or off by her, from what she heard with it and without (no clock decides).
   The Journal tab in sections that fold, each scrolled in its own height.
   Next: each with an instrument, playing together (the human one, she another), and the ear's learning in nodes.
   *Then done* (her music, and her lessons): what she learned kept between sessions (`<memory>.music.json`); an ear
   that hears clarity, brightness and loudness, and learns what each knob does by watching the human turn it (and
   where they like it) and by her own gestures; pads (a knob moved slowly with a long note); call and response at the
   keys; the keys by octaves (Z, X); a playhead on the roll; exact notes on the sequencer; stereo (the bass a little
   left, the melody a little right: `NAME_r` beside a node plays it on the right); MIDI takes written and played again.
   Sixteen lessons, always improvised (modes, chords and progressions, the world's scales, every metre from 3/4 to
   9/8 aksak and 12/8, swing, the twelve tones, then nothing but her ears), each held by a bridle that loosens with
   her own level (guided, accompanied, free; free, she may tune by ratios rather than semitones); a studio where she
   chooses by her learning progress, leaves a lesson too hard for now *for later*, rests when bored, and asks to sleep
   when what she lived weighs enough; her mood (pleasure, surprise, curiosity, boredom); the ratios of the intervals
   beside her taste.
   *Then done*: the tonic hers, every key by the circle of fifths, the key of the human heard (Krumhansl) and
   joined; changes of mode, of key, of both, judged and tasted; jazz chord to chord (chord-scales; ii-V cycles, rhythm
   changes, altered minor ii-V-i, Coltrane changes); the circle of fifths and the comma drawn. The filter's runaway
   found and mended (its poles a sample late in a loop), a fuse on every played network, the sound started again by
   hand; the knobs she turned as a log.
   *Then done*: every note value (sixteenths to wholes, dotted, triplets), strict first then loosened; two notes
   together by counterpoint's first rules; thirty lessons following one another on their own, then chosen by her as
   she grows free (what she lived follows each lesson by its name); an LFO on the synthesiser (filter, vibrato,
   tremolo).
   *Then done*: one voice four times (polyphonic), three oscillators each with FM; the human's keys taking a voice
   of hers and giving it back; the computer's keys by where they are (AZERTY too); notes lit while they sound; rec
   beside play (notes and knobs, played again as they were). Her switch now read on the instrument shown only (a copy
   of its network elsewhere had its own switch on: she went on writing notes into that copy, unheard).
   *Then done*: lessons on the synthesiser itself (each knob explored, said in words, then sounds looked for); an
   arpeggiator; her left hand (chords of her own); envelopes in seconds, knobs by ratios, a double click to reset;
   the window kept maximised; her pace set by her pleasure and her boredom; the human's keys never on a voice the
   sequencer holds.
   *Then done*: the tempo in BPM; a metronome, and her sense of time learned with it; gates kept open let go; the
   instrument's nodes found by their file names (the keys and the arpeggiator were writing to another app's node).
   *Then done*: the network as a flow, lit live while it plays; notes and knobs sent straight to the sound (short
   notes were lost: the machine told the sound only every tenth of a second); the beat drawn on the roll.
   *Then done*: each their instrument (her own synthesiser, locked to the human but for its volume; she never
   touches the human's; she hears both; one tempo); drums (noise, the envelope on the pitch); the flow with small
   points and an oscilloscope on what is heard.
   *Then done*: her surprise measured against her usual spread, and novelty fading with the time spent in the same
   lesson and key (she was never bored: now the same again bores her); her pace shown with its reason; lessons on
   the sequencer and the arpeggiator; guided, her leaps imposed (thirds to ninths) and her notes on the beat; the
   click going on and off said.
   Mended: a delay was not counted as a node of an app, so two copies of the same network shared their delay lines
   (her echo and her reverb poured into the human's): each copy now has its own.
   Next for the flow: a voice or a module folded into one block, opened with a double click.
   **Agreed next** (lots 3 and 4, with Anthony): hearing the computer's sound (a loopback, with consent);
   a catalogue of free MIDI to listen to; phonemes, an inner voice, reading and writing (Wikipedia), a scrabble of
   phonemes; games against herself (go, chess); a sandbox for mathematics in GRML; a corner to rest and dream; her
   inner dialogues; thinking several moves ahead; memory kept compressed, opened when read. Her musical self as nodes
   of the network (for now a file beside it). Microtonal scales as they are
   (maqam quarter tones, pelog and slendro as gamelans tune them), not tempered.
   **To think through with Anthony**: every node and every relation as a potential input and output (a relation's
   strain read as a node, a network that pulses, with feedback, plugged into the sequencer).
   **Agreed next** (with Anthony): Our World in Data as a first source of the world (the next lot); MIDI in, as
   many controllers as plugged, everything in the app controllable by MIDI with MIDI learn everywhere (not only
   music); a real analogue synthesiser with all its modules (oscillators, filters, envelopes, LFOs, VCA, mixer,
   noise...) and a sequencer that drives it (a network apart), besides the keyboard; reverb and delay made as networks, no effect ready-made (one new brick: a node that
   remembers its value N samples ago; an echo is one node and one relation, a reverb a feedback delay network of
   8 to 16 of them, an equaliser or a compressor the same brick with gains); effects the
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
   **Now, before any new function** (with Anthony, the rules in `CLAUDE.md`): make everything that is there work,
   in this order. 0. the window's own tests (*done*: egui_kittest, each bug played again); 1. what breaks and what
   is lost (*done*: a workshop erased leaves nothing, the airlock remembers, one logic for every tab, the way back,
   the inspector's signs, Her named by the human, the eye put away; *next in it*: a walker that tries every menu of
   every screen); 2. widgets and one style, written in GRML (the window an interpreter, as a browser is: *2a done*, the style file; then the widgets, with Anthony: *W1 done* (widgets that hold widgets, kinds of one's own, `tip`, `when`, tips by kind in the settings); *W2 done* (one field, one choice, one toggle, everywhere; `choice` and `field` in GRML); W3 list, tree, table, notifications; W4 text and LaTeX, chart, timeline, scope, steps, roll, transport, picture; W5 the composite widgets in GRML (mixer strip, transport, player, the inspector's list); W6 every app and the window itself moved onto them (the workshop, the space, the studio, the player, the universe, the weather), and the editor of each app made a real one; the style edited in the window, working files from JSON to GRML); 3. every
   screen gone over (the space, the network, the workshop made easy, the code, the insights); 4. the studio (modes
   held, the arpeggiator, the mixer, sound only for the tab in front); 5. then her screen made deep (what was the
   reverie), the universe, the table view, an installer; the starry sky after.
4. **The senses**: images (a pyramid of patches), sounds (bands of frequency), seeing the network pulse, dreaming.
5. **The decentralised network**: the internet transport (no central server), public, private and closed networks,
   signatures, the sas (identity, form, permissions, quarantine, compatibility, earned trust), debates between
   hypotheses, human experts, vaccines.
6. **Apps made of networks**: a word processor, a spreadsheet, a DAW (*first version done*: the studio, its
   transport, synthesisers and drum machines added by the human, a mixer with shared echo and reverb, her hands on
   each instrument by the `hand` word, her own network ELLE in GRML apart from the tools, deterministic, with
   contexts, a first knowledge). Next for her: the human's notes heard by her cochlea alone, her sense of beat as
   an oscillator that entrains, curiosity choosing what to learn, the lessons as contexts, sleep that consolidates
   and forgets, contexts recognised by herself; the drums, the knobs, the sequencer and the arpeggiator as GRML
   modules she plays; the old improviser in Rust removed (its chance with it); a library of modules in the window;
   apps wired together. a physical model of a guitar shared with
   others; programming by networks; flows wired like modules (sound, pictures, video, as in TouchDesigner) on the
   mixer's sources; plug-ins (VST) loading GRM packages.
7. **Further**: a bootable USB key, GRM-OS.
