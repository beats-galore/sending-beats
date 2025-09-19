coreaudio_stream -> receives_input -> writes to rtrb producer -> calls input
notifier of input_worker (resample and effects) input_worker (resample and
effects) -> receives input -> resamples if necessary -> uses processed_output_tx
to send processed_audio mixing_layer -> receives input from ALL input workers ->
mixes them -> sends to all mixed_output_senders (output_worker) output_worker ->
receives mixed output -> applies resampling -> writes to spmc queue
coreaudio_output_stream -> reads spmc -> sends to device
