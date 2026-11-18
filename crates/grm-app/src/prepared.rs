// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 Anthony Charretier
//! What the network view needs of a picture of the world, made once for each new picture (not again at every frame
//! drawn): its points and links, its networks and what joins them, and its packets: the nodes a `repeat` wrote
//! (`elle_e1` .. `elle_e36`), seen from afar as one point holding the form of their equations.

use crate::model::Picture;
use std::collections::{HashMap, HashSet};

/// A family of nodes of one network that differ only by their numbers (one point when seen from afar).
#[derive(Debug, Clone)]
pub struct Packet {
    /// Its name, the numbers said `{k}`: `elle_e{k}`.
    pub name: String,
    pub source: String,
    /// Its nodes (indices in the picture's nodes).
    pub members: Vec<usize>,
    /// How many relations hold only nodes of packets and touch it (they are not drawn one by one from afar).
    pub relations: usize,
    /// The form of its equations: one of them, as written.
    pub form: String,
}

/// One end of a link drawn from afar: a point (its key) or a packet.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum End {
    Item(String),
    Packet(usize),
}

#[derive(Debug, Clone, Default)]
pub struct Prepared {
    /// The shape of the picture it was made from (see `Picture::shape`).
    pub shape: u64,
    /// What it shows of the picture: its nodes and its relations (where they are in it).
    pub nodes: Vec<usize>,
    pub rels: Vec<usize>,
    /// Each node's point (`n:NAME`, or `g:EYE` for an eye's glance input), each relation's (`r:NAME`).
    pub node_key: Vec<String>,
    pub rel_key: Vec<String>,
    /// Every point once, and every link once.
    pub items: Vec<String>,
    pub links: Vec<(String, String)>,
    /// For each relation, the points of its nodes (each once).
    pub rel_ends: Vec<Vec<String>>,
    /// The eyes: (eye, (how many inputs, its source)).
    pub groups: indexmap::IndexMap<String, (usize, String)>,
    /// Its networks, in order, with how many nodes each.
    pub counts: Vec<(String, usize)>,
    /// What joins two networks: how many relations hold nodes of both.
    pub joins: Vec<(String, String, usize)>,
    pub packets: Vec<Packet>,
    /// For each node, its packet.
    pub packet_of: Vec<Option<usize>>,
    /// For each relation, whether all its nodes are in packets (from afar it is not drawn: the packets are).
    pub rel_hidden: Vec<bool>,
    /// From afar: what joins the packets and the points (each once, with how many relations).
    pub far_links: Vec<(End, End, usize)>,
}

/// A node's point: itself, or its eye when it is one of the eye's glance inputs.
pub fn item_of(node: &str) -> String {
    match node.split_once("_glance_") {
        Some((eye, _)) => format!("g:{eye}"),
        None => format!("n:{node}"),
    }
}

/// A name with its numbers said `{k}` (`elle_tl3_12` is `elle_tl{k}_{k}`); None when it has none. The first
/// part of a name is who it is (`t1`, the first track; `t2`, the second): its number stays.
pub fn stem(name: &str) -> Option<String> {
    let (head, rest) = match name.split_once('_') {
        Some((h, r)) => (h, r),
        None => return None,
    };
    let mut out = format!("{head}_");
    let mut digits = false;
    let mut any = false;
    let mut prev_alpha = false;
    for c in rest.chars() {
        if c.is_ascii_digit() && (prev_alpha || digits) {
            if !digits {
                out.push_str("{k}");
                any = true;
            }
            digits = true;
        } else {
            digits = false;
            out.push(c);
        }
        prev_alpha = c.is_alphanumeric() || c == '_';
    }
    any.then_some(out)
}

/// The fewest nodes a packet holds (fewer are seen one by one).
const PACKET_LEAST: usize = 4;

impl Prepared {
    /// What the view needs of this picture: everything, or one source (its relations, and the nodes they touch).
    pub fn of(picture: &Picture, only: Option<&str>, shape: u64) -> Prepared {
        let (nodes, rels): (Vec<usize>, Vec<usize>) = match only {
            None => ((0..picture.nodes.len()).collect(), (0..picture.relations.len()).collect()),
            Some(src) => {
                let rels: Vec<usize> =
                    picture.relations.iter().enumerate().filter(|(_, r)| r.source == src).map(|(i, _)| i).collect();
                let touched: HashSet<&String> = rels.iter().flat_map(|&i| picture.relations[i].nodes.iter()).collect();
                let nodes = picture
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(_, n)| n.source == src || touched.contains(&n.name))
                    .map(|(k, _)| k)
                    .collect();
                (nodes, rels)
            }
        };
        let pic_nodes: Vec<&crate::model::NodeView> = nodes.iter().map(|&k| &picture.nodes[k]).collect();
        let pic_rels: Vec<&crate::model::RelationView> = rels.iter().map(|&i| &picture.relations[i]).collect();
        // (where each node shown is, by its name, among those shown)
        let index: HashMap<&str, usize> = pic_nodes.iter().enumerate().map(|(k, n)| (n.name.as_str(), k)).collect();
        struct View<'a> {
            nodes: Vec<&'a crate::model::NodeView>,
            relations: Vec<&'a crate::model::RelationView>,
        }
        let pic = View { nodes: pic_nodes, relations: pic_rels };
        let node_key: Vec<String> = pic.nodes.iter().map(|n| item_of(&n.name)).collect();
        let rel_key: Vec<String> = pic.relations.iter().map(|r| format!("r:{}", r.name)).collect();
        let mut groups: indexmap::IndexMap<String, (usize, String)> = indexmap::IndexMap::new();
        let mut items: Vec<String> = vec![];
        let mut seen: HashSet<String> = HashSet::new();
        for (n, key) in pic.nodes.iter().zip(&node_key) {
            if let Some(eye) = key.strip_prefix("g:") {
                groups.entry(eye.to_string()).or_insert((0, n.source.clone())).0 += 1;
            }
            if seen.insert(key.clone()) {
                items.push(key.clone());
            }
        }
        let mut links: Vec<(String, String)> = vec![];
        let mut linked: HashSet<(String, String)> = HashSet::new();
        let mut rel_ends: Vec<Vec<String>> = vec![];
        for (r, rk) in pic.relations.iter().zip(&rel_key) {
            items.push(rk.clone());
            let mut ends: Vec<String> = vec![];
            for n in &r.nodes {
                let key = item_of(n);
                if ends.contains(&key) {
                    continue;
                }
                ends.push(key.clone());
                let link = (rk.clone(), key);
                if linked.insert(link.clone()) {
                    links.push(link);
                }
            }
            rel_ends.push(ends);
        }
        // its networks, and what joins them
        let mut counts: Vec<(String, usize)> = vec![];
        let mut at: HashMap<&str, usize> = HashMap::new();
        for n in &pic.nodes {
            match at.get(n.source.as_str()) {
                Some(&k) => counts[k].1 += 1,
                None => {
                    at.insert(&n.source, counts.len());
                    counts.push((n.source.clone(), 1));
                }
            }
        }
        let mut joined: indexmap::IndexMap<(String, String), usize> = indexmap::IndexMap::new();
        for r in &pic.relations {
            let mut srcs: Vec<&str> =
                r.nodes.iter().filter_map(|n| picture.node(n).map(|v| v.source.as_str())).collect();
            srcs.sort();
            srcs.dedup();
            for a in 0..srcs.len() {
                for b in a + 1..srcs.len() {
                    *joined.entry((srcs[a].to_string(), srcs[b].to_string())).or_insert(0) += 1;
                }
            }
        }
        let joins = joined.into_iter().map(|((a, b), n)| (a, b, n)).collect();
        // its packets: a network's nodes that differ only by their numbers, when there are enough of them
        let mut by_stem: indexmap::IndexMap<(String, String), Vec<usize>> = indexmap::IndexMap::new();
        for (k, n) in pic.nodes.iter().enumerate() {
            if node_key[k].starts_with("g:") {
                continue;
            }
            if let Some(s) = stem(&n.name) {
                by_stem.entry((n.source.clone(), s)).or_default().push(k);
            }
        }
        let mut packets: Vec<Packet> = vec![];
        let mut packet_of: Vec<Option<usize>> = vec![None; pic.nodes.len()];
        for ((source, name), members) in by_stem {
            if members.len() < PACKET_LEAST {
                continue;
            }
            for &m in &members {
                packet_of[m] = Some(packets.len());
            }
            packets.push(Packet { name, source, members, relations: 0, form: String::new() });
        }
        let packet_of_name = |n: &str| index.get(n).and_then(|&k| packet_of.get(k).copied().flatten());
        let mut rel_hidden: Vec<bool> = vec![false; pic.relations.len()];
        let mut far: indexmap::IndexMap<(End, End), usize> = indexmap::IndexMap::new();
        for (i, r) in pic.relations.iter().enumerate() {
            let ps: Vec<Option<usize>> = r.nodes.iter().map(|n| packet_of_name(n)).collect();
            let hidden = !ps.is_empty() && ps.iter().all(|p| p.is_some());
            rel_hidden[i] = hidden;
            if hidden {
                // the packets it joins (each pair once), and the form of their equations
                let mut ids: Vec<usize> = ps.iter().flatten().copied().collect();
                ids.sort();
                ids.dedup();
                for &p in &ids {
                    let pk = &mut packets[p];
                    pk.relations += 1;
                    if pk.form.is_empty() {
                        pk.form = r.text.clone();
                    }
                }
                for a in 0..ids.len() {
                    for b in a + 1..ids.len() {
                        *far.entry((End::Packet(ids[a]), End::Packet(ids[b]))).or_insert(0) += 1;
                    }
                }
            } else {
                // drawn as it is: its link to each of its ends, a packet's when the node is in one
                let mut ends: Vec<End> = vec![];
                for (n, p) in r.nodes.iter().zip(&ps) {
                    let e = match p {
                        Some(p) => End::Packet(*p),
                        None => End::Item(item_of(n)),
                    };
                    if !ends.contains(&e) {
                        ends.push(e);
                    }
                }
                for e in ends {
                    *far.entry((End::Item(rel_key[i].clone()), e)).or_insert(0) += 1;
                }
            }
        }
        let far_links = far.into_iter().map(|((a, b), n)| (a, b, n)).collect();
        // the nodes of a packet held near one another (a soft link from each to the next): from afar their packet
        // stands where they are, and near they are seen together
        for pk in &packets {
            for w in pk.members.windows(2) {
                let link = (node_key[w[0]].clone(), node_key[w[1]].clone());
                if linked.insert(link.clone()) {
                    links.push(link);
                }
            }
        }
        Prepared {
            shape,
            nodes,
            rels,
            node_key,
            rel_key,
            items,
            links,
            rel_ends,
            groups,
            counts,
            joins,
            packets,
            packet_of,
            rel_hidden,
            far_links,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{NodeView, RelationView};

    #[test]
    fn a_repeat_is_seen_from_afar_as_one_packet() {
        assert_eq!(stem("elle_tl3_12").as_deref(), Some("elle_tl{k}_{k}"));
        assert_eq!(stem("elle_tot"), None);
        // the track is who it is: its voices are a packet, not the voices of every track together
        assert_eq!(stem("t1_v2_cut").as_deref(), Some("t1_v{k}_cut"));
        let node = |n: &str| NodeView { name: n.into(), source: "app:x".into(), ..Default::default() };
        let mut nodes: Vec<NodeView> = (1..=6).map(|k| node(&format!("x_e{k}"))).collect();
        nodes.push(node("tot"));
        let mut relations: Vec<RelationView> = (1..=6)
            .map(|k| RelationView {
                name: format!("set_e{k}"),
                text: format!("set x_e{k} = 0.9 * x_e{k}"),
                nodes: vec![format!("x_e{k}")],
                source: "app:x".into(),
                ..Default::default()
            })
            .collect();
        relations.push(RelationView {
            name: "all".into(),
            text: "tot = x_e1 + x_e2".into(),
            nodes: vec!["tot".into(), "x_e1".into(), "x_e2".into()],
            source: "app:x".into(),
            ..Default::default()
        });
        let by_name = nodes.iter().enumerate().map(|(k, n)| (n.name.clone(), k)).collect();
        let pic = Picture { nodes, relations, by_name, ..Default::default() };
        let p = Prepared::of(&pic, None, 1);
        assert_eq!(p.packets.len(), 1);
        assert_eq!(p.packets[0].name, "x_e{k}");
        assert_eq!(p.packets[0].members.len(), 6);
        assert_eq!(p.packets[0].relations, 6);
        assert_eq!(p.packets[0].form, "set x_e1 = 0.9 * x_e1");
        // its own relations are not drawn from afar; the one that reaches out is, linked to the packet (once)
        assert_eq!(p.rel_hidden.iter().filter(|h| **h).count(), 6);
        let reach: Vec<&(End, End, usize)> =
            p.far_links.iter().filter(|(a, _, _)| *a == End::Item("r:all".into())).collect();
        assert_eq!(reach.len(), 2, "{reach:?}");
        assert!(reach.iter().any(|(_, b, n)| *b == End::Packet(0) && *n == 1));
        // its links, and its nodes held together (one to the next)
        assert_eq!(p.links.len(), 6 + 3 + 5);
    }
}
