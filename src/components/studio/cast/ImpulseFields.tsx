import { NativeSelect, SimpleGrid, Text, TextInput } from '@mantine/core';

import { color } from '../../../theme/tokens';
import { CAST_SEGMENT_LENGTHS } from '../../../types/cast.types';
import { useStationFields } from './use-station-fields';

type ImpulseFieldsProps = {
  stationId: string;
  isLive: boolean;
};

/**
 * Where an Impulse station is: an origin, a slug, and how long a segment lasts.
 *
 * The endpoint carries its scheme because there is no default that is right —
 * it is https in front of Cloudflare and http in front of a worker running
 * locally, and a bare host cannot say which.
 */
export const ImpulseFields = ({ stationId, isLive }: ImpulseFieldsProps) => {
  const { station, edit, locked } = useStationFields(stationId, isLive);

  if (!station) {
    return null;
  }

  return (
    <>
      <TextInput
        label="INGEST ENDPOINT"
        value={station.endpointUrl ?? ''}
        onChange={(event) => edit({ endpointUrl: event.currentTarget.value })}
        placeholder="https://impulse.example.com"
        {...locked}
      />

      <SimpleGrid cols={2} spacing="xl" verticalSpacing="lg">
        <TextInput
          label="STATION"
          value={station.stationSlug ?? ''}
          onChange={(event) => edit({ stationSlug: event.currentTarget.value })}
          placeholder="shady"
          {...locked}
        />
        <NativeSelect
          label="SEGMENT"
          value={String(station.segmentMs)}
          onChange={(event) => edit({ segmentMs: Number(event.currentTarget.value) })}
          data={CAST_SEGMENT_LENGTHS.map((length) => ({
            value: String(length),
            label: `${length / 1000}s`,
          }))}
          {...(isLive ? { disabled: true } : {})}
        />
      </SimpleGrid>

      {/* Segment length is the only latency control there is here, and it is not
          free: shorter segments mean more requests and a smaller margin before
          a slow one is late. */}
      <Text size="2xs" c={color.textFaint}>
        Listeners hear about {((station.segmentMs / 1000) * 3).toFixed(0)}s behind the mix
      </Text>
    </>
  );
};
