// Loading spinner component with size variants
import React from 'react';
import { LoadingSpinnerProps } from '../../types';

const sizeClasses = {
  sm: 'w-4 h-4',
  md: 'w-8 h-8',
  lg: 'w-12 h-12'
} as const;

export const LoadingSpinner: React.FC<LoadingSpinnerProps> = ({
  size = 'md',
  color = 'text-blue-500'
}) => {
  return (
    <div className="flex items-center justify-center">
      <svg
        className={`animate-spin ${sizeClasses[size]} ${color}`}
        xmlns="http://www.w3.org/2000/svg"
        fill="none"
        viewBox="0 0 24 24"
      >
        <circle
          className="opacity-25"
          cx="12"
          cy="12"
          r="10"
          stroke="currentColor"
          strokeWidth="4"
        />
        <path
          className="opacity-75"
          fill="currentColor"
          d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
        />
      </svg>
    </div>
  );
};

// Full-screen loading component
export const FullScreenLoader: React.FC<{ message?: string }> = ({ 
  message = 'Loading...' 
}) => {
  return (
    <div className="min-h-screen bg-gray-900 flex items-center justify-center">
      <div className="text-center">
        <LoadingSpinner size="lg" color="text-blue-400" />
        <p className="mt-4 text-gray-300">{message}</p>
      </div>
    </div>
  );
};

// Inline loading state for components
export const InlineLoader: React.FC<{ message?: string }> = ({ 
  message = 'Loading...' 
}) => {
  return (
    <div className="flex items-center justify-center py-8">
      <div className="flex items-center space-x-3">
        <LoadingSpinner size="sm" color="text-gray-400" />
        <span className="text-gray-400 text-sm">{message}</span>
      </div>
    </div>
  );
};