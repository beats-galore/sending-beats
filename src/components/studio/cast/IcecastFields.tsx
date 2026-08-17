import { Group, SimpleGrid, TextInput } from '@mantine/core';

import { useStationFields } from './use-station-fields';

type IcecastFieldsProps = {
  stationId: string;
  isLive: boolean;
};

/**
 * Where an Icecast station is: a server, a port, a mount and a user.
 *
 * These four are what a source client asks for and they have no counterpart on
 * the other protocol, which is why they live in their own component rather than
 * being conditionally hidden inside a shared form.
 */
export const IcecastFields = ({ stationId, isLive }: IcecastFieldsProps) => {
  const { station, edit, locked } = useStationFields(stationId, isLive);

  if (!station) {
    return null;
  }

  return (
    <>
      <Group gap="sm" wrap="nowrap" align="flex-end">
        <TextInput
          label="SERVER"
          value={station.serverHost}
          onChange={(event) => edit({ serverHost: event.currentTarget.value })}
          style={{ flex: 1, minWidth: 0 }}
          {...locked}
        />
        <TextInput
          value={String(station.serverPort)}
          onChange={(event) => edit({ serverPort: Number(event.currentTarget.value) || 0 })}
          w={74}
          {...locked}
        />
      </Group>

      <SimpleGrid cols={2} spacing="xl" verticalSpacing="lg">
        <TextInput
          label="MOUNT"
          value={station.mountPoint}
          onChange={(event) => edit({ mountPoint: event.currentTarget.value })}
          {...locked}
        />
        <TextInput
          label="USER"
          value={station.username}
          onChange={(event) => edit({ username: event.currentTarget.value })}
          {...locked}
        />
      </SimpleGrid>
    </>
  );
};
