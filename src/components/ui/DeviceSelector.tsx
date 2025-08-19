// Audio device selector dropdown component
import React, { memo } from 'react';
import { DeviceSelectorProps } from '../../types';

export const DeviceSelector = memo<DeviceSelectorProps>(({
  devices,
  selectedDeviceId,
  onDeviceChange,
  placeholder = 'Select device...',
  disabled = false
}) => {
  const handleChange = (event: React.ChangeEvent<HTMLSelectElement>) => {
    const deviceId = event.target.value;
    if (deviceId && deviceId !== selectedDeviceId) {
      onDeviceChange(deviceId);
    }
  };

  // Find selected device name for display
  const selectedDevice = devices.find(device => device.id === selectedDeviceId);

  return (
    <div className="w-full">
      <select
        value={selectedDeviceId || ''}
        onChange={handleChange}
        disabled={disabled}
        className={[
          'w-full px-3 py-2 text-sm',
          'bg-gray-800 border border-gray-600 rounded',
          'text-gray-200 placeholder-gray-500',
          'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:border-blue-500',
          'transition-colors duration-150',
          disabled 
            ? 'opacity-50 cursor-not-allowed' 
            : 'hover:border-gray-500 cursor-pointer'
        ].join(' ')}
      >
        {/* Placeholder option */}
        <option value="" disabled>
          {placeholder}
        </option>
        
        {/* Device options */}
        {devices.map(device => (
          <option 
            key={device.id} 
            value={device.id}
            className="bg-gray-800 text-gray-200"
          >
            {device.name}
            {device.is_default && ' (Default)'}
          </option>
        ))}
        
        {/* No devices message */}
        {devices.length === 0 && (
          <option value="" disabled className="bg-gray-800 text-gray-500">
            No devices available
          </option>
        )}
      </select>
      
      {/* Device info display */}
      {selectedDevice && (
        <div className="mt-1 text-xs text-gray-500">
          {selectedDevice.host_api} • {selectedDevice.supported_channels[0] || 'Unknown'} ch
          {selectedDevice.is_default && ' • Default'}
        </div>
      )}
      
      {/* Error state */}
      {selectedDeviceId && !selectedDevice && (
        <div className="mt-1 text-xs text-red-400">
          ⚠️ Selected device not found
        </div>
      )}
    </div>
  );
}, (prevProps, nextProps) =>
  prevProps.selectedDeviceId === nextProps.selectedDeviceId &&
  prevProps.disabled === nextProps.disabled &&
  prevProps.devices.length === nextProps.devices.length &&
  prevProps.devices.every((device, index) => 
    device.id === nextProps.devices[index]?.id &&
    device.name === nextProps.devices[index]?.name
  )
);