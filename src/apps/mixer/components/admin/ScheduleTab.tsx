import { Stack, Group, Title, Button, Card, Table, Text, Badge, ActionIcon } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconPlus, IconEdit, IconTrash } from '@tabler/icons-react';
import { memo } from 'react';

type ScheduleItem = {
  id: string;
  title: string;
  dj: string;
  startTime: string;
  endTime: string;
  day: string;
  isActive: boolean;
};

type ScheduleTabProps = {
  schedules: ScheduleItem[];
};

const useStyles = createStyles((theme) => ({
  tableCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
}));

export const ScheduleTab = memo<ScheduleTabProps>(({ schedules }) => {
  const { classes } = useStyles();

  return (
    <Stack gap="lg">
      <Group justify="space-between" align="center">
        <Title order={3} c="blue.4">
          Show Schedule
        </Title>
        <Button leftSection={<IconPlus size={16} />} color="blue">
          Add Show
        </Button>
      </Group>

      <Card className={classes.tableCard} padding={0} withBorder>
        <Table striped highlightOnHover>
          <Table.Thead>
            <Table.Tr>
              <Table.Th>Show</Table.Th>
              <Table.Th>DJ</Table.Th>
              <Table.Th>Day</Table.Th>
              <Table.Th>Time</Table.Th>
              <Table.Th>Status</Table.Th>
              <Table.Th>Actions</Table.Th>
            </Table.Tr>
          </Table.Thead>
          <Table.Tbody>
            {schedules.map((schedule) => (
              <Table.Tr key={schedule.id}>
                <Table.Td>
                  <Text fw={600}>{schedule.title}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">{schedule.dj}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">{schedule.day}</Text>
                </Table.Td>
                <Table.Td>
                  <Text c="dimmed">
                    {schedule.startTime} - {schedule.endTime}
                  </Text>
                </Table.Td>
                <Table.Td>
                  <Badge color={schedule.isActive ? 'green' : 'gray'} variant="light" size="sm">
                    {schedule.isActive ? 'Active' : 'Inactive'}
                  </Badge>
                </Table.Td>
                <Table.Td>
                  <Group gap="xs">
                    <ActionIcon variant="subtle" color="blue">
                      <IconEdit size={16} />
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

ScheduleTab.displayName = 'ScheduleTab';
