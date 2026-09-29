import { useState, useEffect, useMemo, useCallback } from 'react';
import {
  Box,
  Typography,
  TextField,
  InputAdornment,
  List,
  ListItem,
  ListItemButton,
  ListItemText,
  Paper,
  Chip,
  Divider,
  IconButton,
  Button,
} from '@mui/material';
import { Search as SearchIcon, Pencil as EditIcon } from 'lucide-react';
import { useOrganization } from '../contexts/OrganizationContext';
import { useProject } from '../contexts/ProjectContext';
import { useToast } from '../contexts/ToastContext';
import { useSchemaStatus } from '../contexts/SchemaContext';
import {
  eventService,
  type EventSchemaProperty,
  type EventDescription,
  type PropertyDescription,
} from '../services/eventService';
import {
  colorCream,
  colorCream2,
  colorInk60,
  colorBlue,
  colorBluePale,
  colorRosePale,
  colorRoseDark,
} from '../theme/tokens';
import { LoadingSkeleton } from '../components/shared/LoadingSkeleton';
import { DescriptionEditDialog } from '../components/shared/DescriptionEditDialog';

interface MergedProperty extends EventSchemaProperty {
  description: string;
  hasDescription: boolean;
}

interface EventWithProperties {
  eventName: string;
  eventDescription: string;
  hasEventDescription: boolean;
  properties: MergedProperty[];
  propertiesNeedingDescription: number;
}

export default function EventSchemaPage() {
  const { currentOrganization } = useOrganization();
  const { currentProject } = useProject();
  const { showSuccess, showError } = useToast();
  const { refresh: refreshSchemaStatus } = useSchemaStatus();

  const [eventNames, setEventNames] = useState<string[]>([]);
  const [schemaCatalog, setSchemaCatalog] = useState<EventSchemaProperty[]>([]);
  const [eventDescriptions, setEventDescriptions] = useState<Map<string, EventDescription>>(new Map());
  const [propertyDescriptions, setPropertyDescriptions] = useState<Map<string, PropertyDescription>>(new Map());
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedEventName, setSelectedEventName] = useState<string | null>(null);

  // Edit dialog state
  const [editDialogOpen, setEditDialogOpen] = useState(false);
  const [editingType, setEditingType] = useState<'event' | 'property'>('event');
  const [editingProperty, setEditingProperty] = useState<MergedProperty | null>(null);
  const [editLoading, setEditLoading] = useState(false);

  // Permission check - org admin can always edit, project admin can edit their project
  const isOrgAdmin = currentOrganization?.access_level?.includes('admin') ?? false;
  const isProjectAdmin = (currentProject as { access_level?: string })?.access_level?.includes('admin') ?? false;
  const canEdit = isOrgAdmin || isProjectAdmin;

  useEffect(() => {
    if (currentOrganization?.id && currentProject?.id) {
      loadData();
    }
  }, [currentOrganization?.id, currentProject?.id]);

  const loadData = async () => {
    setLoading(true);
    setError(null);
    try {
      const [names, catalog, eventDescs, propDescs] = await Promise.all([
        eventService.getEventNamesFromClickHouse(),
        eventService.getEventSchemaCatalog(),
        eventService.getEventDescriptions(currentProject!.id),
        eventService.getPropertyDescriptions(currentProject!.id),
      ]);
      setEventNames(names);
      setSchemaCatalog(catalog);

      // Convert to maps for quick lookup
      const eventDescMap = new Map<string, EventDescription>();
      for (const desc of eventDescs) {
        eventDescMap.set(desc.event_name, desc);
      }
      setEventDescriptions(eventDescMap);

      const propDescMap = new Map<string, PropertyDescription>();
      for (const desc of propDescs) {
        propDescMap.set(`${desc.event_name}|${desc.property_name}`, desc);
      }
      setPropertyDescriptions(propDescMap);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load events');
    } finally {
      setLoading(false);
    }
  };

  const eventsWithProperties: EventWithProperties[] = useMemo(() => {
    const propertyMap = new Map<string, EventSchemaProperty[]>();
    for (const prop of schemaCatalog) {
      const existing = propertyMap.get(prop.event_name) || [];
      existing.push(prop);
      propertyMap.set(prop.event_name, existing);
    }

    return eventNames.map((eventName) => {
      const eventDesc = eventDescriptions.get(eventName);
      const rawProps = propertyMap.get(eventName) || [];

      const mergedProps: MergedProperty[] = rawProps.map((prop) => {
        const propDesc = propertyDescriptions.get(`${eventName}|${prop.property}`);
        return {
          ...prop,
          description: propDesc?.description || '',
          hasDescription: !!propDesc?.description,
        };
      });

      const propsNeedingDesc = mergedProps.filter((p) => !p.hasDescription).length;

      return {
        eventName,
        eventDescription: eventDesc?.description || '',
        hasEventDescription: !!eventDesc?.description,
        properties: mergedProps,
        propertiesNeedingDescription: propsNeedingDesc,
      };
    });
  }, [eventNames, schemaCatalog, eventDescriptions, propertyDescriptions]);

  const filteredEvents = useMemo(() => {
    if (!searchQuery.trim()) return eventsWithProperties;
    const query = searchQuery.toLowerCase();
    return eventsWithProperties.filter(
      (event) =>
        event.eventName.toLowerCase().includes(query) ||
        event.properties.some(
          (p) =>
            p.property.toLowerCase().includes(query) ||
            p.type.toLowerCase().includes(query)
        )
    );
  }, [eventsWithProperties, searchQuery]);

  const selectedEvent = useMemo(() => {
    if (!selectedEventName) return null;
    return eventsWithProperties.find((e) => e.eventName === selectedEventName) || null;
  }, [eventsWithProperties, selectedEventName]);

  const handleEditEventDescription = useCallback(() => {
    if (!selectedEvent) return;
    setEditingType('event');
    setEditingProperty(null);
    setEditDialogOpen(true);
  }, [selectedEvent]);

  const handleEditPropertyDescription = useCallback((prop: MergedProperty) => {
    setEditingType('property');
    setEditingProperty(prop);
    setEditDialogOpen(true);
  }, []);

  const handleSaveDescription = useCallback(
    async (description: string) => {
      if (!currentProject?.id) return;

      setEditLoading(true);
      try {
        if (editingType === 'event' && selectedEvent) {
          await eventService.upsertEventDescription(
            currentProject.id,
            selectedEvent.eventName,
            description
          );

          // Update local state
          setEventDescriptions((prev) => {
            const next = new Map(prev);
            next.set(selectedEvent.eventName, {
              id: '',
              project_id: currentProject.id,
              event_name: selectedEvent.eventName,
              description,
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            });
            return next;
          });

          showSuccess('Event description updated');
          refreshSchemaStatus();
        } else if (editingType === 'property' && editingProperty) {
          await eventService.upsertPropertyDescription(
            currentProject.id,
            selectedEvent!.eventName,
            editingProperty.property,
            editingProperty.type,
            description
          );

          // Update local state
          setPropertyDescriptions((prev) => {
            const next = new Map(prev);
            const key = `${selectedEvent!.eventName}|${editingProperty.property}`;
            next.set(key, {
              id: '',
              project_id: currentProject.id,
              event_name: selectedEvent!.eventName,
              property_name: editingProperty.property,
              property_type: editingProperty.type,
              description,
              created_at: new Date().toISOString(),
              updated_at: new Date().toISOString(),
            });
            return next;
          });

          showSuccess('Property description updated');
          refreshSchemaStatus();
        }
      } catch (err) {
        const message = err instanceof Error ? err.message : 'Failed to save description';
        showError(message);
        throw err;
      } finally {
        setEditLoading(false);
      }
    },
    [currentProject?.id, editingType, selectedEvent, editingProperty, showSuccess, showError, refreshSchemaStatus]
  );

  const getEditDialogData = useMemo(() => {
    if (editingType === 'event' && selectedEvent) {
      return {
        title: 'Edit Event Description',
        subtitle: selectedEvent.eventName,
        initialValue: selectedEvent.eventDescription,
      };
    }
    if (editingType === 'property' && editingProperty) {
      return {
        title: 'Edit Property Description',
        subtitle: `${selectedEvent?.eventName} → ${editingProperty.property}`,
        initialValue: editingProperty.description,
      };
    }
    return { title: '', subtitle: '', initialValue: '' };
  }, [editingType, selectedEvent, editingProperty]);

  // Calculate stats for header
  const stats = useMemo(() => {
    const totalEvents = eventsWithProperties.length;
    const eventsNeedingDesc = eventsWithProperties.filter((e) => !e.hasEventDescription).length;
    const totalProps = eventsWithProperties.reduce((sum, e) => sum + e.properties.length, 0);
    const propsNeedingDesc = eventsWithProperties.reduce(
      (sum, e) => sum + e.propertiesNeedingDescription,
      0
    );
    return { totalEvents, eventsNeedingDesc, totalProps, propsNeedingDesc };
  }, [eventsWithProperties]);

  if (loading) {
    return (
      <Box sx={{ p: 3 }}>
        <LoadingSkeleton variant="page" />
      </Box>
    );
  }

  if (error) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography color="error">Error: {error}</Typography>
      </Box>
    );
  }

  return (
    <Box sx={{ display: 'flex', flexDirection: 'column', height: '100%' }}>
      <Box sx={{ p: 3, pb: 0 }}>
        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', mb: 3 }}>
          <Typography variant="h5" fontWeight={600}>
            Events Schema Catalog
          </Typography>
          <Box sx={{ display: 'flex', gap: 2 }}>
            {stats.eventsNeedingDesc > 0 && (
              <Chip
                label={`${stats.eventsNeedingDesc} event${stats.eventsNeedingDesc !== 1 ? 's' : ''} missing description`}
                size="small"
                sx={{
                  bgcolor: colorRosePale,
                  color: colorRoseDark,
                  fontWeight: 500,
                }}
              />
            )}
            {stats.propsNeedingDesc > 0 && (
              <Chip
                label={`${stats.propsNeedingDesc} propert${stats.propsNeedingDesc !== 1 ? 'ies' : 'y'} missing description`}
                size="small"
                sx={{
                  bgcolor: colorRosePale,
                  color: colorRoseDark,
                  fontWeight: 500,
                }}
              />
            )}
          </Box>
        </Box>
      </Box>

      <Box sx={{ display: 'flex', flex: 1, overflow: 'hidden', px: 3, pb: 3, gap: 2 }}>
        {/* Left Panel - Events List */}
        <Paper
          sx={{
            width: 320,
            flexShrink: 0,
            display: 'flex',
            flexDirection: 'column',
            border: `1px solid ${colorCream2}`,
            borderRadius: 2,
            overflow: 'hidden',
          }}
        >
          <Box sx={{ p: 2, borderBottom: `1px solid ${colorCream2}` }}>
            <TextField
              placeholder="Search events..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              size="small"
              fullWidth
              InputProps={{
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchIcon size={20} sx={{ color: colorInk60 }} />
                  </InputAdornment>
                ),
              }}
            />
            <Typography variant="caption" color="text.secondary" sx={{ mt: 1, display: 'block' }}>
              {filteredEvents.length} event{filteredEvents.length !== 1 ? 's' : ''}
            </Typography>
          </Box>

          <List sx={{ flex: 1, overflow: 'auto', py: 0 }}>
            {filteredEvents.map((event) => {
              const isSelected = selectedEventName === event.eventName;
              const needsDesc = !event.hasEventDescription;
              return (
                <ListItem key={event.eventName} disablePadding>
                  <ListItemButton
                    selected={isSelected}
                    onClick={() => setSelectedEventName(event.eventName)}
                    sx={{
                      '&.Mui-selected': {
                        bgcolor: colorBluePale,
                        '&:hover': { bgcolor: colorBluePale },
                      },
                    }}
                  >
                    <ListItemText
                      primary={
                        <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                            <Typography fontWeight={500} fontSize={14}>
                              {event.eventName}
                            </Typography>
                            {needsDesc && (
                              <Box
                                sx={{
                                  width: 8,
                                  height: 8,
                                  borderRadius: '50%',
                                  bgcolor: colorRoseDark,
                                }}
                                title="Missing description"
                              />
                            )}
                          </Box>
                          <Chip
                            label={event.properties.length}
                            size="small"
                            sx={{
                              bgcolor: event.properties.length > 0 ? colorBluePale : colorCream2,
                              color: event.properties.length > 0 ? colorBlue : colorInk60,
                              fontWeight: 500,
                              fontSize: 11,
                              height: 20,
                            }}
                          />
                        </Box>
                      }
                    />
                  </ListItemButton>
                </ListItem>
              );
            })}
            {filteredEvents.length === 0 && (
              <Box sx={{ p: 3, textAlign: 'center' }}>
                <Typography color="text.secondary">No events found</Typography>
              </Box>
            )}
          </List>
        </Paper>

        {/* Right Panel - Properties */}
        <Paper
          sx={{
            flex: 1,
            border: `1px solid ${colorCream2}`,
            borderRadius: 2,
            overflow: 'auto',
          }}
        >
          {selectedEvent ? (
            <Box>
              <Box sx={{ p: 3, borderBottom: `1px solid ${colorCream2}`, bgcolor: colorCream }}>
                <Box sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', gap: 3 }}>
                  <Box>
                    <Typography variant="h6" fontWeight={600}>
                      {selectedEvent.eventName}
                    </Typography>
                    <Typography variant="body2" color="text.secondary">
                      {selectedEvent.properties.length} properties
                    </Typography>
                  </Box>
                  <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 1, flexShrink: 0 }}>
                    {selectedEvent.hasEventDescription && (
                      <Typography variant="body2" color="text.primary" sx={{ textAlign: 'right', maxWidth: 300 }}>
                        {selectedEvent.eventDescription}
                      </Typography>
                    )}
                    {canEdit && (
                      <Button
                        size="small"
                        startIcon={<EditIcon size={20} />}
                        onClick={handleEditEventDescription}
                        sx={{ textTransform: 'none' }}
                      >
                        {selectedEvent.hasEventDescription ? 'Edit Description' : 'Add Description'}
                      </Button>
                    )}
                  </Box>
                </Box>
              </Box>

              {selectedEvent.properties.length === 0 ? (
                <Box sx={{ p: 4, textAlign: 'center' }}>
                  <Typography color="text.secondary" sx={{ fontStyle: 'italic' }}>
                    No properties cataloged for this event
                  </Typography>
                </Box>
              ) : (
                <Box sx={{ p: 2 }}>
                  <Box
                    sx={{
                      display: 'grid',
                      gridTemplateColumns: '1fr 120px 1fr 40px',
                      gap: 1,
                      mb: 1,
                      px: 2,
                    }}
                  >
                    <Typography variant="caption" fontWeight={600} color="text.secondary">
                      Property
                    </Typography>
                    <Typography variant="caption" fontWeight={600} color="text.secondary">
                      Type
                    </Typography>
                    <Typography variant="caption" fontWeight={600} color="text.secondary">
                      Description
                    </Typography>
                    <Box />
                  </Box>
                  <Divider sx={{ mb: 1 }} />
                  {selectedEvent.properties.map((prop, index) => (
                    <Box
                      key={prop.property}
                      sx={{
                        display: 'grid',
                        gridTemplateColumns: '1fr 120px 1fr 40px',
                        gap: 1,
                        py: 1.5,
                        px: 2,
                        borderRadius: 1,
                        bgcolor: index % 2 === 0 ? 'transparent' : colorCream,
                        alignItems: 'center',
                      }}
                    >
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        <Typography
                          variant="body2"
                          fontFamily="'Source Code Pro', monospace"
                          fontWeight={500}
                        >
                          {prop.property}
                        </Typography>
                        {!prop.hasDescription && (
                          <Box
                            sx={{
                              width: 6,
                              height: 6,
                              borderRadius: '50%',
                              bgcolor: colorRoseDark,
                            }}
                            title="Missing description"
                          />
                        )}
                      </Box>
                      <Chip
                        label={prop.type}
                        size="small"
                        sx={{
                          bgcolor: colorCream2,
                          fontSize: 12,
                          fontFamily: "'Source Code Pro', monospace",
                          height: 24,
                        }}
                      />
                      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                        {prop.hasDescription ? (
                          <Typography variant="body2" color="text.primary">
                            {prop.description}
                          </Typography>
                        ) : canEdit ? (
                          <Button
                            size="small"
                            startIcon={<EditIcon size={20} />}
                            onClick={() => handleEditPropertyDescription(prop)}
                            sx={{ textTransform: 'none', color: colorInk60, fontSize: 12 }}
                          >
                            Add description
                          </Button>
                        ) : (
                          <Typography
                            variant="body2"
                            color="text.secondary"
                            sx={{ fontStyle: 'italic' }}
                          >
                            No description
                          </Typography>
                        )}
                      </Box>
                      {prop.hasDescription && canEdit && (
                        <IconButton
                          size="small"
                          onClick={() => handleEditPropertyDescription(prop)}
                          sx={{ p: 0.5 }}
                        >
                          <EditIcon size={16} sx={{ color: colorInk60 }} />
                        </IconButton>
                      )}
                    </Box>
                  ))}
                </Box>
              )}
            </Box>
          ) : (
            <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'center', height: '100%' }}>
              <Typography color="text.secondary">Select an event to view its properties</Typography>
            </Box>
          )}
        </Paper>
      </Box>

      <DescriptionEditDialog
        open={editDialogOpen}
        onClose={() => setEditDialogOpen(false)}
        onSave={handleSaveDescription}
        title={getEditDialogData.title}
        subtitle={getEditDialogData.subtitle}
        initialValue={getEditDialogData.initialValue}
      />
    </Box>
  );
}
