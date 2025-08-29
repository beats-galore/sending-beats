### Audio Stream Crunchiness on Bass Frequencies

**Status**: UNRESOLVED  
**Priority**: Medium  
**Date Discovered**: 2025-08-26

**Description**: Despite significant improvements to audio stream quality, there
are still occasional crunches and glitches that occur specifically on bass
frequencies. The audio stream has gotten much better overall but bass-heavy
content still experiences intermittent distortion.

**Investigation Done**:

- Previous fixes have significantly improved audio quality
- Issue is now isolated to bass frequency range
- Problem is intermittent rather than constant

**Next Steps**:

1. Check low-frequency filter and processing in audio effects chain
2. Verify bass EQ band processing for clipping or overflow issues
3. Test with different bass-heavy audio sources to isolate trigger conditions
4. Check compressor/limiter settings - may be too aggressive on low frequencies
5. Analyze buffer sizes and sample processing for low-frequency artifacts
6. Consider if bass frequencies need different processing parameters
7. Test with different bit depths and sample rates for bass content

**Workaround**: Reduce bass levels in EQ or use external audio processing before
input.
