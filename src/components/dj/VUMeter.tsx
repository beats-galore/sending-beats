import { Box } from '@mantine/core';
import { createStyles } from '@mantine/styles';
import { memo } from 'react';

type VUMeterProps = {
  level: number;
};

const useStyles = createStyles((theme) => ({
  vuMeter: {
    height: 80,
    backgroundColor: theme.colors.dark[8],
    borderRadius: theme.radius.sm,
    padding: theme.spacing.xs,
    display: 'flex',
    alignItems: 'flex-end',
    gap: 2,
  },

  vuBar: {
    flex: 1,
    backgroundColor: theme.colors.gray[6],
    borderRadius: 1,
    transition: 'all 0.1s ease',
    minHeight: 2,
  },

  vuBarActive: {
    backgroundColor: theme.colors.blue[5],
  },
}));

export const VUMeter = memo<VUMeterProps>(({ level }) => {
  const { classes } = useStyles();
  const normalizedLevel = Math.max(0, Math.min(1, level / 255));
  const barCount = 20;
  const activeBars = Math.floor(normalizedLevel * barCount);

  const getBarColor = (index: number, isActive: boolean) => {
    if (!isActive) return '#495057';

    if (index > barCount * 0.8) return '#fa5252';
    if (index > barCount * 0.6) return '#fd7e14';
    return '#339af0';
  };

  const getBarHeight = (normalizedLevel: number) => {
    return `${Math.max(10, normalizedLevel * 100)}%`;
  };

  return (
    <Box className={classes.vuMeter}>
      {Array.from({ length: barCount }, (_, i) => (
        <Box
          key={i}
          className={`${classes.vuBar} ${i < activeBars ? classes.vuBarActive : ''}`}
          style={{
            height: getBarHeight(normalizedLevel),
            backgroundColor: getBarColor(i, i < activeBars),
          }}
        />
      ))}
    </Box>
  );
});

VUMeter.displayName = 'VUMeter';
