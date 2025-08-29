For our next feature, I'd like to add the ability to record the audio output of
the mixer to an mp3 or flac file. This should record from the output directly,
and not through the DJ Client.

This should include

- [] ability to start/stop recording without affecting the mixer output
- [] file configuration, with name, format, location, and ability to set file
  metadata (title, artist, album, genre)

If there are any other features that you think are important, or fundamentally
necessary to include on the recording feature, add to the list that i've
provided

For next steps:

I want to rethink the DJ client / mixer integration. Right now these are in
separate tabs, however the main usecase for both is going to be usage at the
same time, so a user can manage their inputs and also see the streaming
connection, update the metadata, and monitor the streamed bitrate.

My ideas for handling these alongside each other are:

2 separate configurable view.

1. Being able to pop out the DJ client into a separate window
2. Configuring it to be inline with the mixer, keeping the current mixer layout
   but adding a tile on the right side of the screen for the DJ Client

Additionally, I think another really cool feature would be to be able to create
an input source that reads from a queue of MP3 or system files. Essentially you
could queue files into a player like VLC, and then xconfigure that as an input
source that writes directly to the stream.

Lay out a development plan, diagnose any potential pitfalls, and digest the work
into small and workable chunks, iterate through them, act as unencumbered as you
can without feedback from me
