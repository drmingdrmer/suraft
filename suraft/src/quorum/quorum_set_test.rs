use maplit::btreeset;

use crate::quorum::QuorumSet;

#[test]
fn test_simple_quorum_set_impl() -> anyhow::Result<()> {
    // Vec as majority quorum set
    {
        let m12345 = vec![1, 2, 3, 4, 5];

        assert!(!m12345.is_quorum([0].iter()));
        assert!(!m12345.is_quorum([0, 1, 2].iter()));
        assert!(!m12345.is_quorum([6, 7, 8].iter()));
        assert!(m12345.is_quorum([1, 2, 3].iter()));
        assert!(m12345.is_quorum([3, 4, 5].iter()));
        assert!(m12345.is_quorum([1, 3, 4, 5].iter()));
    }

    // BTreeSet as majority quorum set
    {
        let m12345 = btreeset! {1,2,3,4,5};

        assert!(!m12345.is_quorum([0].iter()));
        assert!(!m12345.is_quorum([0, 1, 2].iter()));
        assert!(!m12345.is_quorum([6, 7, 8].iter()));
        assert!(m12345.is_quorum([1, 2, 3].iter()));
        assert!(m12345.is_quorum([3, 4, 5].iter()));
        assert!(m12345.is_quorum([1, 3, 4, 5].iter()));
    }

    Ok(())
}
