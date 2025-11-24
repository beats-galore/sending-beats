import { Card, Stack, Title, Grid, TextInput, Button } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconBroadcast } from '@tabler/icons-react';
import { memo } from 'react';

type Metadata = {
  title: string;
  artist: string;
  album: string;
};

type MetadataCardProps = {
  metadata: Metadata;
  onMetadataChange: (metadata: Metadata) => void;
  onUpdateMetadata: () => void;
};

const useStyles = createStyles((theme) => ({
  metadataCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
}));

export const MetadataCard = memo<MetadataCardProps>(
  ({ metadata, onMetadataChange, onUpdateMetadata }) => {
    const { classes } = useStyles();

    const handleFieldChange =
      (field: keyof Metadata) => (event: React.ChangeEvent<HTMLInputElement>) => {
        onMetadataChange({
          ...metadata,
          [field]: event.target.value,
        });
      };

    return (
      <Card className={classes.metadataCard} padding="lg" withBorder>
        <Stack gap="md">
          <Title order={3} c="blue.4">
            Track Metadata
          </Title>

          <Grid>
            <Grid.Col span={{ base: 12, md: 4 }}>
              <TextInput
                label="Title"
                placeholder="Track title"
                value={metadata.title}
                onChange={handleFieldChange('title')}
              />
            </Grid.Col>
            <Grid.Col span={{ base: 12, md: 4 }}>
              <TextInput
                label="Artist"
                placeholder="Artist name"
                value={metadata.artist}
                onChange={handleFieldChange('artist')}
              />
            </Grid.Col>
            <Grid.Col span={{ base: 12, md: 4 }}>
              <TextInput
                label="Album"
                placeholder="Album name"
                value={metadata.album}
                onChange={handleFieldChange('album')}
              />
            </Grid.Col>
          </Grid>

          <Button
            onClick={onUpdateMetadata}
            disabled={!metadata.title || !metadata.artist}
            leftSection={<IconBroadcast size={16} />}
            color="blue"
            variant="light"
          >
            Update Metadata
          </Button>
        </Stack>
      </Card>
    );
  }
);

MetadataCard.displayName = 'MetadataCard';
