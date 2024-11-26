use std::collections::BTreeMap;

use maplit::btreemap;
use maplit::btreeset;

use crate::storage::membership::Membership;
use crate::testing::nid;
use crate::Node;

#[test]
fn test_membership_summary() -> anyhow::Result<()> {
    let m = Membership::new(
        btreemap! {nid(1)=>Node::new(""),nid(2)=>Node::new("")},
    );
    assert_eq!("{1:(),2:()}", m.to_string());

    Ok(())
}

#[test]
fn test_membership() -> anyhow::Result<()> {
    let m123 = Membership::new(btreeset! {nid(1), nid(2), nid(3)});
    assert_eq!(
        vec![nid(1), nid(2), nid(3)],
        m123.node_ids().collect::<Vec<_>>()
    );

    Ok(())
}

#[test]
fn test_membership_with_nodes() -> anyhow::Result<()> {
    let node = Node::default;
    let with_nodes = |nodes| Membership::new(nodes);

    let res = with_nodes(btreemap! {nid(1)=>node(), nid(2)=>node()});
    assert_eq!(
        btreemap! {nid(1)=>node(), nid(2)=>node()},
        res.nodes()
            .map(|(nid, n)| (nid.clone(), n.clone()))
            .collect::<BTreeMap<_, _>>()
    );

    Ok(())
}
