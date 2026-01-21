# Audio Assets Guide

This directory contains audio files for the game's music and sound effects.

## Directory Structure

- `music/` - Background music files (MIDI or pre-rendered audio)
- `sfx/` - Sound effect files (OGG, WAV, or MP3)

## Music System

The game supports two formats for background music:

### 1. MIDI Files (Recommended for Dynamic Music)

Place `.mid` files in the `music/` directory. MIDI files are:
- **Small file size** (typically 10-100 KB)
- **Synthesized in real-time** using Tone.js
- **Dynamic** - can be modified programmatically

**Example:**
```javascript
// To play a MIDI file as background music
playMidi('assets/music/theme.mid', { loop: true });
```

**Where to find free MIDI files:**
- https://freemidi.org/
- https://bitmidi.com/
- https://www.midiworld.com/
- https://musescore.com/ (export as MIDI)

### 2. Pre-rendered Audio (OGG/MP3/WAV)

Place `.ogg`, `.mp3`, or `.wav` files in the `music/` directory for:
- **Higher quality** with specific instruments
- **Faster playback** (no synthesis overhead)
- **Larger file size**

**Example:**
```javascript
// To play pre-rendered audio as background music
playMusic('assets/music/theme.ogg', { loop: true });
```

**Recommended format:** OGG Vorbis (best compression, widely supported)

## Sound Effects

Place sound effect files in the `sfx/` directory. Currently used:

- `attack.ogg` - Played when attacking (Space key or Attack button)
- `gather.ogg` - Played when gathering resources (F key or Gather button)
- `interact.ogg` - Played when interacting with NPCs/objects (E key or Interact button)

**Where to find free sound effects:**
- https://freesound.org/
- https://opengameart.org/
- https://sonniss.com/gameaudiogdc (annual free pack)
- https://mixkit.co/free-sound-effects/

### Adding New Sound Effects

1. Place the audio file in `public/assets/sfx/`
2. Use the `playSfx()` function in your code:

```javascript
playSfx('assets/sfx/your-sound.ogg', { volume: 0.7 });
```

## Audio Formats

### Recommended Formats by Browser Support

| Format | Chrome | Firefox | Safari | Edge | File Size | Quality |
|--------|--------|---------|--------|------|-----------|---------|
| OGG    | ✅     | ✅      | ✅*    | ✅   | Small     | Good    |
| MP3    | ✅     | ✅      | ✅     | ✅   | Medium    | Good    |
| WAV    | ✅     | ✅      | ✅     | ✅   | Large     | Best    |

*Safari 14.1+ supports OGG Vorbis

**Best practice:** Use OGG for most files, with MP3 fallback if needed.

## File Size Guidelines

- **Background Music (MIDI):** 10-100 KB
- **Background Music (Audio):** 500 KB - 3 MB (for 1-2 minute loop)
- **Sound Effects:** 5-50 KB each

## Converting Audio Files

### To OGG (using ffmpeg):
```bash
ffmpeg -i input.mp3 -c:a libvorbis -q:a 5 output.ogg
```

### To compress/reduce size:
```bash
# Lower quality (smaller file)
ffmpeg -i input.ogg -c:a libvorbis -q:a 3 output.ogg

# Higher quality (larger file)
ffmpeg -i input.ogg -c:a libvorbis -q:a 7 output.ogg
```

### Extract audio from video:
```bash
ffmpeg -i video.mp4 -vn -c:a libvorbis -q:a 5 audio.ogg
```

## Generating Simple Sound Effects

You can generate procedural sound effects using:
- **jsfxr** - https://sfxr.me/ (browser-based, export to WAV)
- **ChipTone** - https://sfbgames.itch.io/chiptone
- **Audacity** - Generate tones, add effects, export

## Sample MIDI Integration

To start playing background music when the game loads:

```javascript
// In game.js, after WebSocket connection is established:
async function startBackgroundMusic() {
  await unlockAudioContext();
  if (audioSettings.musicEnabled) {
    // Option 1: Play MIDI
    playMidi('assets/music/theme.mid', { loop: true });
    
    // Option 2: Play pre-rendered audio
    // playMusic('assets/music/theme.ogg', { loop: true });
  }
}

// Call after first user interaction (e.g., on 'welcome' message)
ws.addEventListener('message', (event) => {
  const msg = JSON.parse(event.data);
  if (msg.type === 'welcome') {
    startBackgroundMusic();
  }
});
```

## License Considerations

When adding audio files:
- Ensure you have the right to use them (CC0, CC-BY, purchased license, or self-created)
- Credit original creators if required by the license
- Keep a `CREDITS.txt` file listing all audio sources

## Performance Tips

- **Preload** important sound effects at startup
- **Limit polyphony** for MIDI (max 8-16 simultaneous notes)
- **Use compressed formats** (OGG/MP3) for longer audio
- **Keep loops seamless** - ensure music loops don't have audible gaps
- **Monitor memory** - unload unused audio buffers if needed

## Troubleshooting

### Music doesn't play
- Check browser console for errors
- Ensure audio was unlocked by a user gesture (click/tap)
- Verify file path is correct
- Check that `audioSettings.musicEnabled` is true

### Sound effects are delayed
- Preload buffers at startup using `loadAudioBuffer(url)`
- Reduce file size or use shorter samples
- Check that you're scheduling sounds correctly

### MIDI sounds "tinny" or synthetic
- This is expected with basic synths
- Consider using pre-rendered audio for better quality
- Or implement SoundFont support for realistic instruments

## Future Enhancements

Potential improvements to the audio system:
- SoundFont (.sf2) support for higher-quality MIDI
- Adaptive music (changes based on game state)
- 3D positional audio for spatial effects
- Reverb/echo for environmental ambience
- Volume sliders in UI for fine-tuned control