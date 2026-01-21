# Audio System Documentation

## Overview

The game now features a complete audio system with support for:
- **Background music** (MIDI or pre-rendered audio)
- **Sound effects** for player actions
- **User controls** for toggling music and SFX
- **Volume management** with persistent settings

## Technologies Used

- **Tone.js** (v15.1.22) - WebAudio framework for synthesis and playback
- **@tonejs/midi** (v2.0.28) - MIDI file parsing and playback
- **Web Audio API** - Native browser audio engine

## Architecture

### Audio Manager (`public/game.js`)

The audio system is integrated into `game.js` with the following components:

#### 1. Audio Settings

```javascript
audioSettings = {
  musicEnabled: true,      // Toggle background music on/off
  sfxEnabled: true,        // Toggle sound effects on/off
  musicVolume: 0.45,       // Music volume (0.0 - 1.0)
  sfxVolume: 0.7           // SFX volume (0.0 - 1.0)
}
```

Settings are persisted to `localStorage` and restored on page load.

#### 2. Audio Context Management

```javascript
ensureAudioContext()      // Creates Web Audio context (lazy initialization)
unlockAudioContext()      // Unlocks audio after user gesture (required by browsers)
applyAudioSettings()      // Applies volume/mute settings to audio nodes
```

#### 3. Music Playback

**MIDI Files** (Recommended for small file size):
```javascript
playMidi(url, { loop: true })   // Load and play MIDI with Tone.js synthesis
stopMidi()                      // Stop MIDI playback and dispose resources
updateMidiVolume()              // Update volume for currently playing MIDI
```

**Pre-rendered Audio** (OGG/MP3/WAV):
```javascript
playMusic(url, { loop: true })  // Load and play audio file via Web Audio
stopMusic()                     // Stop audio playback
```

#### 4. Sound Effects

```javascript
playSfx(url, { volume: 1.0 })   // Play one-shot sound effect
```

Sound effects are triggered automatically for:
- **Attack** (Space key or Attack button) → `assets/sfx/attack.ogg`
- **Gather** (F key or Gather button) → `assets/sfx/gather.ogg`
- **Interact** (E key or Interact button) → `assets/sfx/interact.ogg`

## User Interface

### Toggle Buttons

Two new buttons in the top-right HUD:
- **Music** - Toggle background music on/off
- **SFX** - Toggle sound effects on/off

Active state is indicated by:
- Gold accent color (`--accent-2`)
- Visual glow effect
- `aria-pressed` attribute for accessibility

### Styling

Button styles are defined in `public/styles.css`:
```css
#music-toggle, #sfx-toggle {
  /* Styled panels with pointer events */
}

#music-toggle.active, #sfx-toggle.active {
  /* Gold highlight when enabled */
}
```

## File Structure

```
public/
├── assets/
│   ├── music/           # Background music files
│   │   ├── theme.mid    # (Add your MIDI file here)
│   │   └── theme.ogg    # (Or pre-rendered audio)
│   ├── sfx/             # Sound effect files
│   │   ├── attack.ogg
│   │   ├── gather.ogg
│   │   └── interact.ogg
│   └── AUDIO_README.md  # Detailed audio asset guide
├── game.js              # Audio system implementation
├── index.html           # Tone.js + @tonejs/midi CDN scripts
└── styles.css           # Audio toggle button styles
```

## How MIDI Playback Works

1. **Load MIDI file** using `@tonejs/midi`:
   ```javascript
   const midi = await Midi.fromUrl('assets/music/theme.mid');
   ```

2. **Create synthesizers** for each track:
   ```javascript
   const synth = new Tone.PolySynth(Tone.Synth, {
     maxPolyphony: 8,
     oscillator: { type: 'triangle' },
     envelope: { attack: 0.005, decay: 0.1, sustain: 0.4, release: 0.3 }
   });
   ```

3. **Schedule notes** using `Tone.Part`:
   ```javascript
   const part = new Tone.Part((time, note) => {
     synth.triggerAttackRelease(note.name, note.duration, time, note.velocity);
   }, noteArray);
   ```

4. **Start transport** for playback:
   ```javascript
   part.start(0);
   Tone.getTransport().start();
   ```

## Browser Autoplay Policy

Modern browsers block audio until a user gesture occurs. The audio system handles this by:

1. Requiring `unlockAudioContext()` to be called from a user event
2. Auto-unlocking when music/SFX toggles are clicked
3. Attempting to unlock on first `welcome` message (after WebSocket connection)

## Adding Background Music

### Option 1: MIDI (Small, Dynamic)

1. Place a `.mid` file in `public/assets/music/`
2. Uncomment in `game.js` (around line 2526):
   ```javascript
   playMidi('assets/music/theme.mid', { loop: true });
   ```

**Free MIDI sources:**
- https://freemidi.org/
- https://bitmidi.com/
- https://musescore.com/ (export as MIDI)

### Option 2: Pre-rendered Audio (High Quality)

1. Place an `.ogg` or `.mp3` file in `public/assets/music/`
2. Use in `game.js`:
   ```javascript
   playMusic('assets/music/theme.ogg', { loop: true });
   ```

**Convert to OGG using ffmpeg:**
```bash
ffmpeg -i input.mp3 -c:a libvorbis -q:a 5 output.ogg
```

## Adding Sound Effects

1. Place `.ogg`, `.wav`, or `.mp3` files in `public/assets/sfx/`
2. Modify `playSfxForAction()` in `game.js` to reference your files:
   ```javascript
   case 'attack':
     playSfx('assets/sfx/attack.ogg', { volume: 0.6 }).catch(() => {});
     break;
   ```

**Free SFX sources:**
- https://freesound.org/
- https://opengameart.org/
- https://mixkit.co/free-sound-effects/

**Generate procedural SFX:**
- https://sfxr.me/ (jsfxr)
- https://sfbgames.itch.io/chiptone

## Performance Considerations

### MIDI Playback
- **Polyphony limited to 8 notes per track** to reduce CPU usage
- **Caching:** MIDI files are cached in `midiCache` Map
- **Resource cleanup:** Synths and parts are properly disposed on stop

### Pre-rendered Audio
- **Audio buffers cached** in `audioBuffers` Map (reuse for looping)
- **Compressed formats:** OGG Vorbis offers good quality at small size

### Sound Effects
- **One-shot playback:** Each SFX creates a temporary source node
- **Error handling:** Silent failures if files don't exist (`.catch(() => {})`)

## Customization

### Adjust Synth Sound (for MIDI)

Edit the synth configuration in `playMidi()` (around line 550):

```javascript
const synth = new Tone.PolySynth(Tone.Synth, {
  maxPolyphony: 8,
  oscillator: { 
    type: 'triangle'  // Try: 'sine', 'square', 'sawtooth', 'triangle'
  },
  envelope: { 
    attack: 0.005,    // Note attack time (seconds)
    decay: 0.1,       // Decay time
    sustain: 0.4,     // Sustain level (0-1)
    release: 0.3      // Release time
  },
});
```

### Adjust Volume Levels

Edit default settings in `game.js` (around line 378):

```javascript
const defaultAudioSettings = {
  musicEnabled: true,
  sfxEnabled: true,
  musicVolume: 0.45,  // 0.0 (silent) to 1.0 (full)
  sfxVolume: 0.7
};
```

## Internationalization

Audio toggle labels support translation:

```javascript
// English
musicToggle: 'Toggle music',
sfxToggle: 'Toggle sound effects',

// German
musicToggle: 'Musik umschalten',
sfxToggle: 'Soundeffekte umschalten',
```

## Accessibility

- **ARIA attributes:** Buttons have `aria-pressed` and `aria-label`
- **Keyboard accessible:** Buttons can be focused and activated with keyboard
- **Visual feedback:** Active state clearly indicated with color/glow

## Troubleshooting

### Music doesn't play
- Check browser console for errors
- Verify file exists at the specified path
- Ensure `audioSettings.musicEnabled` is `true`
- Confirm audio context was unlocked (click Music toggle)

### MIDI sounds synthetic
- This is expected with basic Tone.js synths
- Use pre-rendered audio for studio-quality sound
- Or implement SoundFont support (advanced)

### Sound effects are delayed
- Preload important SFX at game start
- Reduce file size (use compressed OGG)
- Check network tab for slow downloads

### No sound in Safari
- Safari requires explicit user gesture before audio
- Click the Music or SFX toggle button first
- Check Safari's audio autoplay settings

## Future Enhancements

Potential improvements:
- **Volume sliders** for fine-tuned control
- **SoundFont (.sf2) support** for realistic MIDI instruments
- **Adaptive music** that changes based on game state (exploration vs combat)
- **3D positional audio** for spatial SFX
- **Reverb/ambience** for environmental immersion
- **Music crossfading** for smooth transitions
- **Audio compression** using AudioWorklet

## API Reference

### Functions

| Function | Parameters | Description |
|----------|-----------|-------------|
| `ensureAudioContext()` | - | Creates Web Audio context if needed |
| `unlockAudioContext()` | - | Resumes suspended audio context |
| `playMidi(url, options)` | `url: string, { loop: boolean }` | Play MIDI file |
| `stopMidi()` | - | Stop MIDI playback |
| `playMusic(url, options)` | `url: string, { loop: boolean }` | Play audio file |
| `stopMusic()` | - | Stop audio playback |
| `playSfx(url, options)` | `url: string, { volume: number }` | Play sound effect |
| `applyAudioSettings()` | - | Apply volume/mute settings |
| `saveAudioSettings()` | - | Persist settings to localStorage |

### Settings Object

```typescript
interface AudioSettings {
  musicEnabled: boolean;   // Toggle music on/off
  sfxEnabled: boolean;     // Toggle SFX on/off
  musicVolume: number;     // 0.0 - 1.0
  sfxVolume: number;       // 0.0 - 1.0
}
```

## Credits

- **Tone.js** - https://tonejs.github.io/
- **@tonejs/midi** - https://github.com/Tonejs/Midi
- **Web Audio API** - https://developer.mozilla.org/en-US/docs/Web/API/Web_Audio_API