// Professional toggle button component for mixer controls
import React, { memo } from 'react';
import { ToggleButtonProps, COLORS } from '../../types';

const variantColors = {
  default: {
    active: COLORS.BUTTON.ACTIVE,
    inactive: COLORS.BUTTON.DEFAULT
  },
  success: {
    active: COLORS.BUTTON.SUCCESS,
    inactive: COLORS.BUTTON.DEFAULT
  },
  warning: {
    active: COLORS.BUTTON.WARNING,
    inactive: COLORS.BUTTON.DEFAULT
  },
  danger: {
    active: COLORS.BUTTON.DANGER,
    inactive: COLORS.BUTTON.DEFAULT
  }
} as const;

export const ToggleButton = memo<ToggleButtonProps>(({
  label,
  pressed,
  onChange,
  variant = 'default',
  disabled = false
}) => {
  const colors = variantColors[variant];
  
  const buttonClasses = [
    'px-3 py-1.5 text-xs font-medium rounded transition-all duration-150',
    'border border-gray-600',
    'focus:outline-none focus:ring-2 focus:ring-blue-500 focus:ring-opacity-50',
    'select-none cursor-pointer',
    disabled ? 'opacity-50 cursor-not-allowed' : 'hover:border-gray-500',
    pressed 
      ? 'text-white shadow-inner' 
      : 'text-gray-300 hover:text-white'
  ].join(' ');

  const buttonStyle = {
    backgroundColor: pressed ? colors.active : colors.inactive,
    borderColor: pressed ? colors.active : undefined
  };

  const handleClick = () => {
    if (!disabled) {
      onChange(!pressed);
    }
  };

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (!disabled && (event.key === 'Enter' || event.key === ' ')) {
      event.preventDefault();
      onChange(!pressed);
    }
  };

  return (
    <button
      className={buttonClasses}
      style={buttonStyle}
      onClick={handleClick}
      onKeyDown={handleKeyDown}
      disabled={disabled}
      role="switch"
      aria-checked={pressed}
      aria-label={`${label} ${pressed ? 'on' : 'off'}`}
      tabIndex={disabled ? -1 : 0}
    >
      {label}
    </button>
  );
}, (prevProps, nextProps) =>
  prevProps.pressed === nextProps.pressed &&
  prevProps.disabled === nextProps.disabled &&
  prevProps.variant === nextProps.variant
);