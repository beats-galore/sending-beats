// Professional Error Boundary using react-error-boundary with Mantine UI
import {
  Container,
  Title,
  Text,
  Button,
  Group,
  Stack,
  Alert,
  Code,
  Collapse,
  Paper,
  Divider,
} from '@mantine/core';
import {
  IconAlertTriangle,
  IconRefresh,
  IconBug,
  IconChevronDown,
  IconChevronUp,
} from '@tabler/icons-react';
import { useState } from 'react';
import type { ErrorInfo } from 'react';
import { ErrorBoundary as ReactErrorBoundary } from 'react-error-boundary';

import type { FallbackProps } from 'react-error-boundary';

// Error fallback component with professional Mantine styling
function ErrorFallback({ error, resetErrorBoundary }: FallbackProps) {
  const [showDetails, setShowDetails] = useState(true); // Auto-expanded as requested

  const handleReload = () => {
    window.location.reload();
  };

  const handleReset = () => {
    resetErrorBoundary();
  };

  return (
    <Container
      fluid
      style={{
        minHeight: '100vh',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        background: 'linear-gradient(135deg, #1a1b23 0%, #2d1b69 100%)',
      }}
      p="xl"
    >
      <Paper
        p="xl"
        radius="lg"
        shadow="xl"
        style={{
          maxWidth: 800,
          width: '100%',
          border: '1px solid #e03131',
        }}
      >
        <Stack gap="lg">
          {/* Header */}
          <Group justify="center">
            <IconAlertTriangle size={48} color="#e03131" />
          </Group>

          <Stack gap="md" align="center">
            <Title order={1} c="red" ta="center">
              Audio Mixer Error
            </Title>
            <Text size="lg" c="dimmed" ta="center">
              Something went wrong with the audio mixer interface
            </Text>
          </Stack>

          <Divider />

          {/* Error Details Section */}
          <Alert
            icon={<IconBug size={20} />}
            title="Technical Information"
            color="red"
            variant="light"
          >
            <Stack gap="sm">
              <Group
                justify="space-between"
                style={{ cursor: 'pointer' }}
                onClick={() => setShowDetails(!showDetails)}
              >
                <Text fw={500}>Error Details</Text>
                {showDetails ? <IconChevronUp size={16} /> : <IconChevronDown size={16} />}
              </Group>

              <Collapse in={showDetails}>
                <Stack gap="md">
                  {/* Error Message */}
                  <Stack gap="xs">
                    <Text size="sm" fw={500} c="red.7">
                      Error Message:
                    </Text>
                    <Code
                      block
                      p="md"
                      style={{
                        background: '#1a1b23',
                        color: '#ff6b6b',
                        fontSize: '14px',
                        lineHeight: 1.4,
                      }}
                    >
                      {error.message}
                    </Code>
                  </Stack>

                  {/* Stack Trace */}
                  {error.stack && (
                    <Stack gap="xs">
                      <Text size="sm" fw={500} c="red.7">
                        Stack Trace:
                      </Text>
                      <Code
                        block
                        p="md"
                        style={{
                          background: '#1a1b23',
                          color: '#c1c2c5',
                          fontSize: '12px',
                          lineHeight: 1.3,
                          maxHeight: '300px',
                          overflowY: 'auto',
                        }}
                      >
                        {error.stack}
                      </Code>
                    </Stack>
                  )}

                  {/* Error Name and Additional Info */}
                  <Stack gap="xs">
                    <Text size="sm" fw={500} c="red.7">
                      Error Type:
                    </Text>
                    <Code style={{ background: '#1a1b23', color: '#ffd43b' }}>
                      {error.name || 'Unknown Error'}
                    </Code>
                  </Stack>
                </Stack>
              </Collapse>
            </Stack>
          </Alert>

          <Divider />

          {/* Action Buttons */}
          <Group justify="center" gap="lg">
            <Button
              leftSection={<IconRefresh size={16} />}
              color="red"
              variant="filled"
              size="lg"
              onClick={handleReset}
            >
              Try Again
            </Button>

            <Button
              leftSection={<IconRefresh size={16} />}
              color="gray"
              variant="outline"
              size="lg"
              onClick={handleReload}
            >
              Reload App
            </Button>
          </Group>

          {/* Additional Help */}
          <Text size="sm" c="dimmed" ta="center">
            If this problem persists, please check the console for more details or restart the
            application.
          </Text>
        </Stack>
      </Paper>
    </Container>
  );
}

// Error logging function
function logError(error: Error, errorInfo: ErrorInfo) {
  console.group('🚨 ErrorBoundary caught an error:');
  console.error('Error:', error);
  console.error('Error Info:', errorInfo);
  if (errorInfo.componentStack) {
    console.error('Component Stack:', errorInfo.componentStack);
  }
  console.groupEnd();
}

// Main ErrorBoundary export
export function ErrorBoundary({ children }: { children: React.ReactNode }) {
  return (
    <ReactErrorBoundary
      FallbackComponent={ErrorFallback}
      onError={logError}
      onReset={() => {
        // Optional: Add any cleanup logic here
        console.debug('ErrorBoundary reset triggered');
      }}
    >
      {children}
    </ReactErrorBoundary>
  );
}

// Legacy export for backward compatibility
export { ErrorBoundary as default };
