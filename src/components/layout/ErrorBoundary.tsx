// Error boundary component for mixer application
import React, { Component, ReactNode } from 'react';
import { ErrorBoundaryState } from '../../types';

type ErrorBoundaryProps = {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: React.ErrorInfo) => void;
};

export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error('ErrorBoundary caught an error:', error, errorInfo);
    
    // Call optional error callback
    this.props.onError?.(error, errorInfo);
  }

  render() {
    if (this.state.hasError) {
      // Custom fallback UI or default error message
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="min-h-screen bg-gray-900 flex items-center justify-center p-4">
          <div className="bg-red-900/20 border border-red-500 rounded-lg p-6 max-w-md w-full text-center">
            <div className="text-red-400 text-6xl mb-4">⚠️</div>
            <h2 className="text-red-300 text-xl font-semibold mb-4">
              Audio Mixer Error
            </h2>
            <p className="text-gray-300 mb-4">
              Something went wrong with the audio mixer interface.
            </p>
            {this.state.error && (
              <details className="text-left mb-4">
                <summary className="text-red-400 cursor-pointer mb-2">
                  Technical Details
                </summary>
                <pre className="text-xs text-gray-400 bg-black/30 p-3 rounded overflow-auto">
                  {this.state.error.message}
                  {this.state.error.stack && (
                    <>
                      {'\n\n'}
                      {this.state.error.stack}
                    </>
                  )}
                </pre>
              </details>
            )}
            <button
              onClick={() => window.location.reload()}
              className="bg-red-600 hover:bg-red-700 text-white px-4 py-2 rounded transition-colors"
            >
              Reload Application
            </button>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}