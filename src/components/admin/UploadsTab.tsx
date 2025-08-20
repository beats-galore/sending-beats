import {
  Stack,
  Group,
  Title,
  Button,
  Card,
  Table,
  Text,
  Badge,
  ActionIcon,
} from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconPlus, IconPlayerPlay, IconTrash } from '@tabler/icons-react';
import { memo } from 'react';

type UploadedTrack = {
  id: string;
  title: string;
  artist: string;
  album: string;
  duration: string;
  fileSize: string;
  uploadDate: string;
  status: 'processing' | 'ready' | 'error';
};

type UploadsTabProps = {
  uploads: UploadedTrack[];
};

const useStyles = createStyles((theme) => ({
  tableCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
}));

export const UploadsTab = memo<UploadsTabProps>(({ uploads }) => {
  const { classes } = useStyles();

  const getStatusColor = (status: UploadedTrack['status']) => {
    switch (status) {
      case 'ready':
        return 'green';
      case 'processing':
        return 'yellow';
      case 'error':
        return 'red';
      default:
        return 'gray';
    }
  };

  return (
    <Stack gap="lg">
      <Group justify="space-between" align="center">
        <Title order={3} c="blue.4">
          Music Library
        </Title>
        <Button leftSection={<IconPlus size={16} />} color="blue">
          Upload Track
        </Button>
      </Group>

      <Card className={classes.tableCard} padding={0} withBorder>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Track</Table.Th>
              <Table.Th>Artist</Table.Th>
              <Table.Th>Album</Table.Th>
              <Table.Th>Duration</Table.Th>
              <Table.Th>Size</Table.Th>
              <Table.Th>Status</Table.Th>
              <Table.Th>Actions</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {uploads.map((track) => (
              <Table.Tr key={track.id}>
                <Table.Td>
                  <Text fw={600}>{track.title}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">{track.artist}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">{track.album}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">{track.duration}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">{track.fileSize}</Text>
                </Table.Td>
                <Table.Td>
                  <Badge
                    color={getStatusColor(track.status)}
                    variant="light"
                    size="sm"
                  >
                    {track.status}
                  </Badge>
                </Table.Td>
                <Table.Td>
                  <Group gap="xs">
                    <ActionIcon variant="subtle" color="blue">
                      <IconPlayerPlay size={16} />
                    </ActionIcon>
                    <ActionIcon variant="subtle" color="red">
                      <IconTrash size={16} />
                    </ActionIcon>
                  </Group>
                </Table.Td>
              </Table.Tr>
            ))}
          </Table.Tbody>
        </Table>
      </Card>
    </Stack>
  );
});

UploadsTab.displayName = 'UploadsTab';