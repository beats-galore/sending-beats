import { useEffect, useRef, useState } from 'react';

export type LogEntry = {
  id: number;
  at: string;
  tag: 'OK' | 'INFO' | 'WARN';
  message: string;
};

const SERIES_LENGTH = 30;

type CastTelemetryInput = {
  isLive: boolean;
  listeners: number | null;
  bitrate: number;
  variableBitrate: boolean;
  lastError: string | null;
};

/**
 * Listener history and a connection log, both assembled on the client.
 *
 * The server reports current state, not history — so the trend line and the log
 * are built from the transitions this session actually observed. They start
 * empty on launch rather than back-filling numbers nobody measured.
 */
export const useCastTelemetry = ({
  isLive,
  listeners,
  bitrate,
  variableBitrate,
  lastError,
}: CastTelemetryInput) => {
  const [series, setSeries] = useState<number[]>([]);
  const [log, setLog] = useState<LogEntry[]>([]);
  const nextId = useRef(0);
  const wasLive = useRef(false);
  const lastLoggedError = useRef<string | null>(null);

  const append = (tag: LogEntry['tag'], message: string) => {
    setLog((entries) =>
      [
        ...entries,
        {
          id: nextId.current++,
          at: new Date().toLocaleTimeString(undefined, { hour12: false }),
          tag,
          message,
        },
      ].slice(-40)
    );
  };

  useEffect(() => {
    if (isLive === wasLive.current) {
      return;
    }
    wasLive.current = isLive;

    if (isLive) {
      append('OK', 'Connected to the stream server');
      append('INFO', `Encoder started — MP3 ${bitrate} ${variableBitrate ? 'VBR' : 'CBR'}`);
    } else {
      append('INFO', 'Disconnected');
      setSeries([]);
    }
    // `bitrate` and `variableBitrate` are read for the message only; a change to
    // either should not re-log a connection that never dropped.
    // oxlint-disable-next-line react-hooks/exhaustive-deps
  }, [isLive]);

  useEffect(() => {
    if (!lastError || lastError === lastLoggedError.current) {
      return;
    }
    lastLoggedError.current = lastError;
    append('WARN', lastError);
  }, [lastError]);

  useEffect(() => {
    if (!isLive || listeners === null) {
      return;
    }
    setSeries((current) => [...current, listeners].slice(-SERIES_LENGTH));
  }, [isLive, listeners]);

  return { series, log, logEvent: append };
};
