import React, { useState } from 'react';

type ScheduleItem = {
  id: string;
  title: string;
  dj: string;
  startTime: string;
  endTime: string;
  day: string;
  isActive: boolean;
};

type AnalyticsData = {
  currentListeners: number;
  peakListeners: number;
  totalStreamTime: string;
  topTracks: { title: string; artist: string; plays: number }[];
};

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

const AdminPanel: React.FC = () => {
  const [activeTab, setActiveTab] = useState<'dashboard' | 'schedule' | 'uploads' | 'analytics'>(
    'dashboard'
  );
  const [schedules, _setSchedules] = useState<ScheduleItem[]>([
    {
      id: '1',
      title: 'Morning Mix',
      dj: 'DJ Luna',
      startTime: '08:00',
      endTime: '10:00',
      day: 'Monday',
      isActive: true,
    },
    {
      id: '2',
      title: 'Afternoon Vibes',
      dj: 'DJ Max',
      startTime: '14:00',
      endTime: '16:00',
      day: 'Wednesday',
      isActive: true,
    },
  ]);

  const [analytics] = useState<AnalyticsData>({
    currentListeners: 127,
    peakListeners: 342,
    totalStreamTime: '1,247 hours',
    topTracks: [
      { title: 'Midnight Groove', artist: 'Luna & The Stars', plays: 156 },
      { title: 'Electric Dreams', artist: 'Neon Pulse', plays: 134 },
      { title: 'Ocean Waves', artist: 'Chill Collective', plays: 98 },
    ],
  });

  const [uploads, _setUploads] = useState<UploadedTrack[]>([
    {
      id: '1',
      title: 'Summer Nights',
      artist: 'Chill Collective',
      album: 'Ocean Waves',
      duration: '3:45',
      fileSize: '8.2 MB',
      uploadDate: '2024-01-15',
      status: 'ready',
    },
    {
      id: '2',
      title: 'Neon City',
      artist: 'Electric Dreams',
      album: 'Cyberpunk Vibes',
      duration: '4:12',
      fileSize: '9.1 MB',
      uploadDate: '2024-01-14',
      status: 'processing',
    },
  ]);

  const [isLive, setIsLive] = useState(false);
  const [currentDJ, _setCurrentDJ] = useState('DJ Luna');

  const TabButton: React.FC<{ tab: string; label: string; icon: string }> = ({
    tab,
    label,
    icon,
  }) => (
    <button
      onClick={() => setActiveTab(tab as any)}
      className={`flex items-center gap-2 px-4 py-2 rounded-lg transition-colors ${
        activeTab === tab ? 'bg-brand text-white' : 'text-surface-light hover:text-white'
      }`}
    >
      <span className="text-lg">{icon}</span>
      {label}
    </button>
  );

  const DashboardTab: React.FC = () => (
    <div className="space-y-6">
      {/* Live Status */}
      <div className="bg-surface rounded-xl p-6">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-xl font-display text-brand">Live Status</h3>
          <div className="flex items-center gap-2">
            <div
              className={`w-3 h-3 rounded-full ${isLive ? 'bg-accent animate-pulse' : 'bg-surface-light'}`}
            />
            <span className="text-sm text-surface-light">{isLive ? 'ON AIR' : 'OFF AIR'}</span>
          </div>
        </div>
        <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
          <div className="bg-surface-light rounded-lg p-4">
            <div className="text-2xl font-display text-brand">{analytics.currentListeners}</div>
            <div className="text-sm text-surface-light">Current Listeners</div>
          </div>
          <div className="bg-surface-light rounded-lg p-4">
            <div className="text-2xl font-display text-accent">{analytics.peakListeners}</div>
            <div className="text-sm text-surface-light">Peak Listeners</div>
          </div>
          <div className="bg-surface-light rounded-lg p-4">
            <div className="text-2xl font-display text-brand">{currentDJ}</div>
            <div className="text-sm text-surface-light">Current DJ</div>
          </div>
        </div>
        <button
          onClick={() => setIsLive(!isLive)}
          className={`mt-4 px-6 py-2 rounded-lg font-medium transition-colors ${
            isLive
              ? 'bg-accent hover:bg-accent-light text-white'
              : 'bg-brand hover:bg-brand-light text-white'
          }`}
        >
          {isLive ? 'Go Off Air' : 'Go Live'}
        </button>
      </div>

      {/* Quick Actions */}
      <div className="bg-surface rounded-xl p-6">
        <h3 className="text-xl font-display text-brand mb-4">Quick Actions</h3>
        <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
          <button className="bg-surface-light hover:bg-brand/20 rounded-lg p-4 text-left transition-colors">
            <div className="text-2xl mb-2">📅</div>
            <div className="font-medium">Schedule</div>
            <div className="text-sm text-surface-light">Manage shows</div>
          </button>
          <button className="bg-surface-light hover:bg-brand/20 rounded-lg p-4 text-left transition-colors">
            <div className="text-2xl mb-2">📊</div>
            <div className="font-medium">Analytics</div>
            <div className="text-sm text-surface-light">View stats</div>
          </button>
          <button className="bg-surface-light hover:bg-brand/20 rounded-lg p-4 text-left transition-colors">
            <div className="text-2xl mb-2">🎵</div>
            <div className="font-medium">Upload</div>
            <div className="text-sm text-surface-light">Add music</div>
          </button>
          <button className="bg-surface-light hover:bg-brand/20 rounded-lg p-4 text-left transition-colors">
            <div className="text-2xl mb-2">⚙️</div>
            <div className="font-medium">Settings</div>
            <div className="text-sm text-surface-light">Configure</div>
          </button>
        </div>
      </div>
    </div>
  );

  const ScheduleTab: React.FC = () => (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <h3 className="text-xl font-display text-brand">Show Schedule</h3>
        <button className="bg-brand hover:bg-brand-light text-white px-4 py-2 rounded-lg transition-colors">
          + Add Show
        </button>
      </div>

      <div className="bg-surface rounded-xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-surface-light">
              <tr>
                <th className="text-left p-4 text-surface-light font-medium">Show</th>
                <th className="text-left p-4 text-surface-light font-medium">DJ</th>
                <th className="text-left p-4 text-surface-light font-medium">Day</th>
                <th className="text-left p-4 text-surface-light font-medium">Time</th>
                <th className="text-left p-4 text-surface-light font-medium">Status</th>
                <th className="text-left p-4 text-surface-light font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {schedules.map((schedule) => (
                <tr key={schedule.id} className="border-b border-surface">
                  <td className="p-4">
                    <div className="font-medium">{schedule.title}</div>
                  </td>
                  <td className="p-4 text-surface-light">{schedule.dj}</td>
                  <td className="p-4 text-surface-light">{schedule.day}</td>
                  <td className="p-4 text-surface-light">
                    {schedule.startTime} - {schedule.endTime}
                  </td>
                  <td className="p-4">
                    <span
                      className={`px-2 py-1 rounded-full text-xs ${
                        schedule.isActive
                          ? 'bg-brand/20 text-brand'
                          : 'bg-surface-light text-surface-light'
                      }`}
                    >
                      {schedule.isActive ? 'Active' : 'Inactive'}
                    </span>
                  </td>
                  <td className="p-4">
                    <button className="text-brand hover:text-brand-light mr-2">Edit</button>
                    <button className="text-accent hover:text-accent-light">Delete</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );

  const UploadsTab: React.FC = () => (
    <div className="space-y-6">
      <div className="flex justify-between items-center">
        <h3 className="text-xl font-display text-brand">Music Library</h3>
        <button className="bg-brand hover:bg-brand-light text-white px-4 py-2 rounded-lg transition-colors">
          + Upload Track
        </button>
      </div>

      <div className="bg-surface rounded-xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full">
            <thead className="bg-surface-light">
              <tr>
                <th className="text-left p-4 text-surface-light font-medium">Track</th>
                <th className="text-left p-4 text-surface-light font-medium">Artist</th>
                <th className="text-left p-4 text-surface-light font-medium">Album</th>
                <th className="text-left p-4 text-surface-light font-medium">Duration</th>
                <th className="text-left p-4 text-surface-light font-medium">Size</th>
                <th className="text-left p-4 text-surface-light font-medium">Status</th>
                <th className="text-left p-4 text-surface-light font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {uploads.map((track) => (
                <tr key={track.id} className="border-b border-surface">
                  <td className="p-4">
                    <div className="font-medium">{track.title}</div>
                  </td>
                  <td className="p-4 text-surface-light">{track.artist}</td>
                  <td className="p-4 text-surface-light">{track.album}</td>
                  <td className="p-4 text-surface-light">{track.duration}</td>
                  <td className="p-4 text-surface-light">{track.fileSize}</td>
                  <td className="p-4">
                    <span
                      className={`px-2 py-1 rounded-full text-xs ${
                        track.status === 'ready'
                          ? 'bg-brand/20 text-brand'
                          : track.status === 'processing'
                            ? 'bg-accent/20 text-accent'
                            : 'bg-surface-light text-surface-light'
                      }`}
                    >
                      {track.status}
                    </span>
                  </td>
                  <td className="p-4">
                    <button className="text-brand hover:text-brand-light mr-2">Play</button>
                    <button className="text-accent hover:text-accent-light">Delete</button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );

  const AnalyticsTab: React.FC = () => (
    <div className="space-y-6">
      <h3 className="text-xl font-display text-brand">Analytics</h3>

      {/* Top Tracks */}
      <div className="bg-surface rounded-xl p-6">
        <h4 className="text-lg font-display text-brand mb-4">Top Tracks</h4>
        <div className="space-y-3">
          {analytics.topTracks.map((track, index) => (
            <div
              key={index}
              className="flex items-center justify-between p-3 bg-surface-light rounded-lg"
            >
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 bg-brand rounded-full flex items-center justify-center text-white font-bold text-sm">
                  {index + 1}
                </div>
                <div>
                  <div className="font-medium">{track.title}</div>
                  <div className="text-sm text-surface-light">{track.artist}</div>
                </div>
              </div>
              <div className="text-brand font-medium">{track.plays} plays</div>
            </div>
          ))}
        </div>
      </div>

      {/* Stream Stats */}
      <div className="bg-surface rounded-xl p-6">
        <h4 className="text-lg font-display text-brand mb-4">Stream Statistics</h4>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="bg-surface-light rounded-lg p-4">
            <div className="text-3xl font-display text-brand">{analytics.totalStreamTime}</div>
            <div className="text-sm text-surface-light">Total Stream Time</div>
          </div>
          <div className="bg-surface-light rounded-lg p-4">
            <div className="text-3xl font-display text-accent">{analytics.peakListeners}</div>
            <div className="text-sm text-surface-light">Peak Listeners</div>
          </div>
        </div>
      </div>
    </div>
  );

  return (
    <div className="bg-surface rounded-2xl p-6 max-w-6xl mx-auto">
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-2xl font-display text-brand">Admin Panel</h2>
        <div className="flex items-center gap-2">
          <div
            className={`w-3 h-3 rounded-full ${isLive ? 'bg-accent animate-pulse' : 'bg-surface-light'}`}
          />
          <span className="text-sm text-surface-light">{isLive ? 'Live' : 'Offline'}</span>
        </div>
      </div>

      {/* Tab Navigation */}
      <div className="flex gap-2 mb-6 overflow-x-auto">
        <TabButton tab="dashboard" label="Dashboard" icon="📊" />
        <TabButton tab="schedule" label="Schedule" icon="📅" />
        <TabButton tab="uploads" label="Uploads" icon="🎵" />
        <TabButton tab="analytics" label="Analytics" icon="📈" />
      </div>

      {/* Tab Content */}
      {activeTab === 'dashboard' && <DashboardTab />}
      {activeTab === 'schedule' && <ScheduleTab />}
      {activeTab === 'uploads' && <UploadsTab />}
      {activeTab === 'analytics' && <AnalyticsTab />}
    </div>
  );
};

export default AdminPanel;
