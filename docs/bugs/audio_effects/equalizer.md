### Equalizer Effect Shows "No Effects Added"

**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**: When adding an equalizer effect to a channel, the UI still
displays "No effects added" instead of showing the equalizer controls.

**Investigation Done**:

- Issue reported by user during testing

**Next Steps**:

1. Check effect addition logic in frontend components
2. Verify effect state management and UI updates
3. Test if effect is actually being applied to audio (might be UI-only issue)
4. Check effect component rendering conditions

**Workaround**: Effect may still be working despite UI not showing it - test
with audio.
