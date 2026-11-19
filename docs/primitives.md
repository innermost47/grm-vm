# The primitives still missing

Everything that can be built from the primitives the machine already draws is in `widgets/` (GRML), shown in the
gallery app (`apps/gallery.grm`). What follows cannot be built that way. Each is a new primitive, drawn by the
engine as a browser draws a button: minimal, written once, called from GRML, taking its look from the style
(`style/style.grm`), saying its words to whoever cannot see it (screen readers, the window's tests).

Each is written here as it would be called. Nothing below exists yet, but the 3D view (§ 9, lot P2a).

## 1. Lists that come from the world

```
list sources = world sources          # the networks the world holds, kept up to date
list nodes = world nodes of typed     # the nodes of one network
list sounds = world sounds            # what can be heard
list workshops = world workshops
list files = world files apps         # the files of a folder
```

A list the world fills, read again when the world changes. Everything a list can do it does (`repeat`, `sum`,
widgets made for each item). It is what ties the window to the network: what the inspector, the space and the
workshops show is a list of the world.

## 2. Words that say values

```
widget k: text "key: {elle_tonic:note} · bar {tr_bars}.{tr_beats}"
widget t: text "{speed:0.1} m/s" fr "{speed:0.1} m/s"
```

`{NODE}` is the node's value as it is now (from the sound when the sound computes it); `{NODE:0.1}` a number with
its decimals, `{NODE:note}` a note's name, `{NODE:time}` minutes and seconds. `fr "..."` gives the French words (the
window's language chooses).

## 3. `list`: a list to see and choose from (a sidebar, an aside)

```
widget networks: list sources selected chosen_source filter "networks" menu source_menu
```

One row per item (its words, its sign, its menu), always scrolled by its own bar, Ctrl+F to filter, the chosen one
written to a node (or to a list of one), keys up and down. A tree is a list whose items are lists (`tree`).

## 4. `menu`: what a right click offers

```
menu source_menu
  item "to the front" icon focus say "focus {item}"
  item "inspect" icon search say "select {item}" when {item}_exists
end
widget n: list nodes menu source_menu
```

Each item says a line (`{item}`: the one clicked); an item that cannot do anything now is greyed (`when`).

## 5. `popup`: a window above the others

```
widget eq: popup "the equation" open eq_open
  widget e: text "{master_equation}"
end
```

Shown while its node is 1; Escape and its cross set it to 0. What it holds is laid out as a column.

## 6. `editor`: code that is written

```
widget code: editor file apps/studio.grm
```

Its colours by what each word is (a node, a relation, a widget, a number, a comment), Ctrl+F, Tab completes from
the world's names, each error said at its line, undo and redo. The editor of every app, the code tab, `space.grm`.

## 7. `table`: rows and columns

```
widget t: table rows sources columns "name|nodes|relations"
```

Cells written by hand when they are inputs; a column whose cells follow from others by a relation (a spreadsheet's
formula is a relation). Sorted by a click on a column's title.

## 8. `timeline`: time, and what happened in it

```
widget epochs: timeline uni_t from 0 to 4.4e17 marks epochs
```

An axis of time (zoomed by the wheel, moved by a drag), its marks (a list: their names and their times), the node
it sets when it is clicked (where one is).

## 9. `view3d`: a world in three dimensions, to look at and handle (done, lot P2a)

```
widget sky: view3d b in bodies: x {b}_x y {b}_y z {b}_z radius {b}_r hue {b}_hue selected chosen_body
```

Shown in the gallery, page `3d`. Done: orbit, pan, zoom to the pointer, double click, `F`, the views along the
axes, flat or perspective; the grid, the axes, the spheres, the names on hover; a click chooses, Escape lets go, the
arrows and Page up / down move the one chosen. Coming with the universe (P2b), which needs them: lines between
objects (a list of pairs), several chosen (shift, a box), a card on hover as the network shows.

What a 3D view of a good piece of software does:

- **Looking**: orbit (drag), pan (right drag, two fingers), zoom to the pointer (wheel, pinch), double click to
  centre on what is under the pointer, fit everything (`F`), the faces of a cube to look along an axis (top, front,
  side), perspective or flat.
- **Seeing**: a grid on the ground, the three axes, a light that follows the eye (and one fixed), the objects as
  spheres, points, lines between them (a list of pairs), their names on hover, a card on hover as the network
  shows.
- **Handling**: click to choose (written to a node), shift to add, a box to choose many, a chosen object moved by
  arrows on its axes (its x, y, z written back to its nodes), Escape to let go.
- **Time**: the positions are nodes, so what moves is the network; the view only draws it.

The universe app is then written in GRML from it: an aside at the left (`widgets/layout.grm`) with the list of
bodies, the view in the middle, the timeline of its epochs under it.

## 10. Notifications

```
notify "the studio is saved" kind quiet
```

Said by a line, shown in the corner a few seconds, of a kind the settings can silence.
