from mido import Message, MidiFile, MidiTrack, MetaMessage

mid = MidiFile(ticks_per_beat=480)

# Tempo track
tempo_track = MidiTrack()
mid.tracks.append(tempo_track)
tempo_track.append(MetaMessage('set_tempo', tempo=666667))  # 90 BPM

# Soft pad track (warm, calming bed)
pad = MidiTrack()
mid.tracks.append(pad)
pad.append(Message('program_change', program=90, time=0))  # Warm pad (Synth)

pad_chords = [
    [62, 65, 69],  # D minor
    [60, 64, 67],  # C major
    [55, 59, 62],  # G major
    [57, 60, 64],  # A minor
]

for chord in pad_chords * 2:
    for note in chord:
        pad.append(Message('note_on', note=note, velocity=40, time=0))
    pad.append(Message('note_off', note=chord[0], velocity=0, time=960))
    pad.append(Message('note_off', note=chord[1], velocity=0, time=0))
    pad.append(Message('note_off', note=chord[2], velocity=0, time=0))

# Gentle melody track (longer, varied phrases)
melody = MidiTrack()
mid.tracks.append(melody)
melody.append(Message('program_change', program=73, time=0))  # Flute

phrase_a = [69, 71, 72, 71, 69, 67, 65, 67]  # A B C B A G F G
phrase_b = [72, 74, 76, 74, 72, 71, 69, 67]  # C D E D C B A G
phrase_c = [67, 69, 71, 72, 71, 69, 67, 65]  # G A B C B A G F

for phrase in [phrase_a, phrase_b, phrase_a, phrase_c]:
    for i, note in enumerate(phrase):
        duration = 960 if i % 4 == 3 else 480
        melody.append(Message('note_on', note=note, velocity=55, time=0))
        melody.append(Message('note_off', note=note, velocity=0, time=duration))

# Soft harp-like arpeggios for movement
arp = MidiTrack()
mid.tracks.append(arp)
arp.append(Message('program_change', program=46, time=0))  # Harp

arp_patterns = [
    [62, 65, 69, 74],
    [60, 64, 67, 72],
    [55, 59, 62, 67],
    [57, 60, 64, 69],
]

for pattern in arp_patterns * 4:
    for note in pattern:
        arp.append(Message('note_on', note=note, velocity=35, time=0))
        arp.append(Message('note_off', note=note, velocity=0, time=240))

# Save
file_path = "calm_spheric_loop.mid"
mid.save(file_path)

file_path
