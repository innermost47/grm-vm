# Bugs found and mended

Each bug found on the way: what was seen, why, the test that plays it again, the lot that mended it.

| Seen | Why | Test | Lot |
|---|---|---|---|
| `node m = 2*pi` was refused (not a number) | a value was a literal or `pi` alone, never a little arithmetic with constants; a value with a space (`tau / 4`) did not even reach it | `the_constants_are_known_everywhere` | A |
| `relate r2: f = m *` was taken, silently | an operator with nothing after it was skipped | `an_equation_cut_short_is_said_so` | A |
| `nod x = 1` in a workshop: nothing said, nothing done | a line whose word is not of the language was not a statement, and nothing said so | `a_workshop_proposes_and_waits_for_a_human`, `the_workshop_is_its_grml_screen_and_its_editor_says_what_is_wrong` | A |
| `workshop say WS ask x` said nothing at the prompt | `ask` was answered by the window only | `every_choice_of_the_workshop_menus_does_what_it_says` | A |
| A line taken back made a workshop forget its rows and its laws | the workshop was made again from an empty world | `a_workshop_proposes_and_waits_for_a_human` (said again keeps them) | A |
| A law found in a workshop's table was written without asking, and never among its lines (taking a line back lost it) | the workshop merged what it found as the main network does | `the_network_proposes_and_a_human_takes_or_refuses` | A |
| Right-click choices in a workshop that did not do what they said | the workshop's network used the main network's menu, then a menu of two | `every_choice_of_the_workshop_menus_does_what_it_says` | A |
| `load FILE.csv --into NAME` without columns: no column found | `--into` was taken for a column | `the_network_proposes_and_a_human_takes_or_refuses` | A |
| The airlock's door filled with the accent | a fill left out by the colour rule | `nothing_is_filled_with_a_colour_words_cannot_stand_on` | A |
| « Sa valeur » on a node of the workshop seemed to do nothing | its field opened in the inspector only, out of sight when the panel was closed or on another tab | `its_value_is_changed_in_the_workshop_and_the_inspector_keeps_its_width_and_closes` | A.1 |
| Clicking in the inspector widened it to more than half the screen | the value field asked for more width than its row had; the panel grew with it, frame after frame, and kept the width once clicked | same test (its width holds) | A.1 |
| The inspector still widened when going from Journal to Inspector | the side panel took its width from what it held (the widest line of a tab) | same test (every tab, its width holds) | A.2 |
| The cross of the workshop's part in the inspector did nothing | the workshop drawn again set it back at once | same test (closed, it stays closed; a node asked for brings it back) | A.1 |
| A scroll bar on the workshop on a tall screen | the screen's rows had a fixed height | seen in capture (a tall window: no bar) | A.1 |
| Orange, then white, words on yellow (an app's tab with errors, the day variant) | words on the accent chose white or dark by a threshold | `the_words_on_what_is_filled_read_in_every_variant_and_on_any_colour` | P2a |
| A 3D view opened before its nodes were computed stayed fitted to nothing | fitted once, at the first frame | `a_view_follows_its_objects_until_the_human_moves_it` | P2a |

## Seen, not mended yet

- A node written with a value (`node m = 70`) is a belief, not a measurement: a relation that makes a new node (its
  value 1) pulls it (`m` became 3.3 when `relate circle: perimeter = 2 * pi * m` was written). The language says so
  (« part de 70 »), and *its value* in the workshop writes a measurement when others depend on the node; whether a
  node made by a relation should be unknown (not 1) is a question for the engine (the parity with the Python
  machine holds it as it is).
- The chosen tab of an app's `tabs` is not kept between sessions.
