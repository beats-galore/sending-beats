### Cannot Re-add Effects After Removal

**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**: Once an effect is removed from a channel, it cannot be
re-added. The effect option becomes unavailable or non-functional.

**Investigation Done**:

- Issue reported by user during testing

**Next Steps**:

1. Check effect removal logic and state cleanup
2. Verify if removed effects are properly reset/reinitialized
3. Test effect availability tracking in UI state
4. Check for memory leaks or incomplete cleanup preventing re-addition

**Workaround**: Restart application to reset effect availability.
