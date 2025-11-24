import { Card, Stack, Title, Switch, Select, Text, Group, Badge, Tooltip } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { IconWaveSquare, IconInfoCircle } from '@tabler/icons-react';
import { memo } from 'react';

type BitrateInfo = {
  current_bitrate: number;
  available_bitrates: number[];
  codec: string;
  is_variable_bitrate: boolean;
  vbr_quality: number; // 0-9 for MP3 VBR
  actual_bitrate: number | null;
};

type VariableBitrateCardProps = {
  bitrateInfo: BitrateInfo;
  onVariableBitrateChange: (enabled: boolean, quality: number) => void;
  disabled?: boolean;
};

const useStyles = createStyles((theme) => ({
  vbrCard: {
    backgroundColor: theme.colors.dark[6],
    border: `1px solid ${theme.colors.dark[4]}`,
  },
  
  bitrateDisplay: {
    display: 'flex',
    alignItems: 'center',
    gap: theme.spacing.xs,
  },
  
  actualBitrate: {
    fontFamily: theme.fontFamilyMonospace,
    fontSize: theme.fontSizes.lg,
    fontWeight: 700,
  },
}));

const getVbrQualityInfo = (quality: number): { label: string; description: string; avgBitrate: string } => {
  switch (quality) {
    case 0: return { label: 'V0 - Highest', description: 'Best quality, largest files', avgBitrate: '~245 kbps' };
    case 1: return { label: 'V1 - Near Highest', description: 'Excellent quality', avgBitrate: '~225 kbps' };
    case 2: return { label: 'V2 - High', description: 'High quality (recommended)', avgBitrate: '~190 kbps' };
    case 3: return { label: 'V3 - Good', description: 'Good quality', avgBitrate: '~175 kbps' };
    case 4: return { label: 'V4 - Standard', description: 'Standard quality', avgBitrate: '~165 kbps' };
    case 5: return { label: 'V5 - Medium', description: 'Medium quality', avgBitrate: '~130 kbps' };
    case 6: return { label: 'V6 - Lower', description: 'Lower quality, smaller files', avgBitrate: '~115 kbps' };
    case 7: return { label: 'V7 - Low', description: 'Low quality', avgBitrate: '~100 kbps' };
    case 8: return { label: 'V8 - Very Low', description: 'Very low quality', avgBitrate: '~85 kbps' };
    case 9: return { label: 'V9 - Lowest', description: 'Minimal quality, smallest files', avgBitrate: '~65 kbps' };
    default: return { label: 'Unknown', description: 'Unknown quality', avgBitrate: 'Unknown' };
  }
};

const vbrQualityOptions = [
  { value: '0', label: 'V0 - Highest (~245 kbps)' },
  { value: '1', label: 'V1 - Near Highest (~225 kbps)' },
  { value: '2', label: 'V2 - High (~190 kbps)' },
  { value: '3', label: 'V3 - Good (~175 kbps)' },
  { value: '4', label: 'V4 - Standard (~165 kbps)' },
  { value: '5', label: 'V5 - Medium (~130 kbps)' },
  { value: '6', label: 'V6 - Lower (~115 kbps)' },
  { value: '7', label: 'V7 - Low (~100 kbps)' },
  { value: '8', label: 'V8 - Very Low (~85 kbps)' },
  { value: '9', label: 'V9 - Lowest (~65 kbps)' },
];

export const VariableBitrateCard = memo<VariableBitrateCardProps>(
  ({ bitrateInfo, onVariableBitrateChange, disabled = false }) => {
    const { classes } = useStyles();
    const qualityInfo = getVbrQualityInfo(bitrateInfo.vbr_quality);
    
    const handleVbrToggle = (enabled: boolean) => {
      onVariableBitrateChange(enabled, bitrateInfo.vbr_quality);
    };
    
    const handleQualityChange = (value: string | null) => {
      if (value) {
        const quality = parseInt(value);
        onVariableBitrateChange(bitrateInfo.is_variable_bitrate, quality);
      }
    };

    return (
      <Card className={classes.vbrCard} padding="lg" withBorder>
        <Stack gap="md">
          <Group justify="space-between" align="center">
            <Group gap="xs">
              <IconWaveSquare size={20} color="#9775fa" />
              <Title order={4} c="violet.4">
                Variable Bitrate (VBR)
              </Title>
              <Tooltip 
                label="VBR automatically adjusts bitrate based on audio complexity - simple audio uses less, complex audio uses more"
                position="top"
                withArrow
                multiline
                w={250}
              >
                <IconInfoCircle size={16} color="#868e96" />
              </Tooltip>
            </Group>
            
            <Switch
              checked={bitrateInfo.is_variable_bitrate}
              onChange={(event) => handleVbrToggle(event.currentTarget.checked)}
              disabled={disabled}
              size="md"
              color="violet"
            />
          </Group>
          
          {bitrateInfo.is_variable_bitrate && (
            <>
              <Stack gap="xs">
                <Group justify="space-between">
                  <Text size="sm" c="dimmed">
                    Quality Level
                  </Text>
                  <Badge color="violet" variant="light" size="sm">
                    {qualityInfo.label}
                  </Badge>
                </Group>
                
                <Select
                  value={bitrateInfo.vbr_quality.toString()}
                  onChange={handleQualityChange}
                  data={vbrQualityOptions}
                  disabled={disabled}
                  color="violet"
                />
                
                <Text size="xs" c="dimmed" ta="center">
                  {qualityInfo.description} • Average: {qualityInfo.avgBitrate}
                </Text>
              </Stack>
              
              <Group justify="space-between" align="center">
                <Text size="sm" c="dimmed">
                  Current Stream:
                </Text>
                <Group className={classes.bitrateDisplay}>
                  {bitrateInfo.actual_bitrate ? (
                    <>
                      <Text className={classes.actualBitrate} c="violet.4">
                        {bitrateInfo.actual_bitrate} kbps
                      </Text>
                      <Badge color="green" variant="dot" size="sm">
                        Live VBR
                      </Badge>
                    </>
                  ) : (
                    <Text className={classes.actualBitrate} c="gray.5">
                      {qualityInfo.avgBitrate} (avg)
                    </Text>
                  )}
                </Group>
              </Group>
            </>
          )}
          
          {!bitrateInfo.is_variable_bitrate && (
            <Text size="sm" c="dimmed" ta="center">
              Using constant bitrate: {bitrateInfo.current_bitrate} kbps ({bitrateInfo.codec})
            </Text>
          )}
        </Stack>
      </Card>
    );
  }
);

VariableBitrateCard.displayName = 'VariableBitrateCard';