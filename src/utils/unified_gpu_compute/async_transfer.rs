//! Asynchronous GPU-to-CPU transfer methods with double buffering.

use super::construction::UnifiedGPUCompute;
use anyhow::Result;
use cust::memory::CopyDestination;

impl UnifiedGPUCompute {
    pub fn get_node_positions_async(&mut self) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        if !self.pos_transfer_pending {
            self.start_position_transfer_async()?;

            return Ok(self.get_current_position_buffer());
        }

        let event_idx = if self.current_pos_buffer { 1 } else { 0 };
        match self.transfer_events[event_idx].query()? {
            cust::event::EventStatus::Ready => {
                self.pos_transfer_pending = false;
                self.current_pos_buffer = !self.current_pos_buffer;

                self.start_position_transfer_async()?;

                Ok(self.get_current_position_buffer())
            }
            cust::event::EventStatus::NotReady => Ok(self.get_current_position_buffer()),
        }
    }

    pub fn get_node_velocities_async(&mut self) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        if !self.vel_transfer_pending {
            self.start_velocity_transfer_async()?;

            return Ok(self.get_current_velocity_buffer());
        }

        let event_idx = if self.current_vel_buffer { 1 } else { 0 };
        match self.transfer_events[event_idx].query()? {
            cust::event::EventStatus::Ready => {
                self.vel_transfer_pending = false;
                self.current_vel_buffer = !self.current_vel_buffer;

                self.start_velocity_transfer_async()?;

                Ok(self.get_current_velocity_buffer())
            }
            cust::event::EventStatus::NotReady => Ok(self.get_current_velocity_buffer()),
        }
    }

    fn start_position_transfer_async(&mut self) -> Result<()> {
        if self.pos_transfer_pending {
            return Ok(());
        }

        let target_buffer = !self.current_pos_buffer;
        let event_idx = if target_buffer { 1 } else { 0 };

        let (target_x, target_y, target_z) = if target_buffer {
            (
                &mut self.host_pos_buffer_b.0,
                &mut self.host_pos_buffer_b.1,
                &mut self.host_pos_buffer_b.2,
            )
        } else {
            (
                &mut self.host_pos_buffer_a.0,
                &mut self.host_pos_buffer_a.1,
                &mut self.host_pos_buffer_a.2,
            )
        };

        target_x.resize(self.allocated_nodes, 0.0);
        target_y.resize(self.allocated_nodes, 0.0);
        target_z.resize(self.allocated_nodes, 0.0);

        self.pos_in_x.copy_to(target_x)?;
        self.pos_in_y.copy_to(target_y)?;
        self.pos_in_z.copy_to(target_z)?;

        self.transfer_events[event_idx].record(&self.transfer_stream)?;

        self.pos_transfer_pending = true;
        Ok(())
    }

    fn start_velocity_transfer_async(&mut self) -> Result<()> {
        if self.vel_transfer_pending {
            return Ok(());
        }

        let target_buffer = !self.current_vel_buffer;
        let event_idx = if target_buffer { 1 } else { 0 };

        let (target_x, target_y, target_z) = if target_buffer {
            (
                &mut self.host_vel_buffer_b.0,
                &mut self.host_vel_buffer_b.1,
                &mut self.host_vel_buffer_b.2,
            )
        } else {
            (
                &mut self.host_vel_buffer_a.0,
                &mut self.host_vel_buffer_a.1,
                &mut self.host_vel_buffer_a.2,
            )
        };

        target_x.resize(self.allocated_nodes, 0.0);
        target_y.resize(self.allocated_nodes, 0.0);
        target_z.resize(self.allocated_nodes, 0.0);

        self.vel_in_x.copy_to(target_x)?;
        self.vel_in_y.copy_to(target_y)?;
        self.vel_in_z.copy_to(target_z)?;

        self.transfer_events[event_idx].record(&self.transfer_stream)?;

        self.vel_transfer_pending = true;
        Ok(())
    }

    fn get_current_position_buffer(&self) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let (mut x, mut y, mut z) = if self.current_pos_buffer {
            (
                self.host_pos_buffer_b.0.clone(),
                self.host_pos_buffer_b.1.clone(),
                self.host_pos_buffer_b.2.clone(),
            )
        } else {
            (
                self.host_pos_buffer_a.0.clone(),
                self.host_pos_buffer_a.1.clone(),
                self.host_pos_buffer_a.2.clone(),
            )
        };

        x.truncate(self.num_nodes);
        y.truncate(self.num_nodes);
        z.truncate(self.num_nodes);

        (x, y, z)
    }

    fn get_current_velocity_buffer(&self) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
        let (mut x, mut y, mut z) = if self.current_vel_buffer {
            (
                self.host_vel_buffer_b.0.clone(),
                self.host_vel_buffer_b.1.clone(),
                self.host_vel_buffer_b.2.clone(),
            )
        } else {
            (
                self.host_vel_buffer_a.0.clone(),
                self.host_vel_buffer_a.1.clone(),
                self.host_vel_buffer_a.2.clone(),
            )
        };

        x.truncate(self.num_nodes);
        y.truncate(self.num_nodes);
        z.truncate(self.num_nodes);

        (x, y, z)
    }

    pub fn sync_all_transfers(&mut self) -> Result<()> {
        if self.pos_transfer_pending {
            let event_idx = if !self.current_pos_buffer { 1 } else { 0 };
            self.transfer_events[event_idx].synchronize()?;
            self.pos_transfer_pending = false;
            self.current_pos_buffer = !self.current_pos_buffer;
        }

        if self.vel_transfer_pending {
            let event_idx = if !self.current_vel_buffer { 1 } else { 0 };
            self.transfer_events[event_idx].synchronize()?;
            self.vel_transfer_pending = false;
            self.current_vel_buffer = !self.current_vel_buffer;
        }

        Ok(())
    }

    pub fn start_async_download_positions(&mut self) -> Result<()> {
        if self.pos_transfer_pending {
            return Ok(());
        }

        let target_buffer = !self.current_pos_buffer;
        let event_idx = if target_buffer { 1 } else { 0 };

        let (target_x, target_y, target_z) = if target_buffer {
            (
                &mut self.host_pos_buffer_b.0,
                &mut self.host_pos_buffer_b.1,
                &mut self.host_pos_buffer_b.2,
            )
        } else {
            (
                &mut self.host_pos_buffer_a.0,
                &mut self.host_pos_buffer_a.1,
                &mut self.host_pos_buffer_a.2,
            )
        };

        target_x.resize(self.num_nodes, 0.0);
        target_y.resize(self.num_nodes, 0.0);
        target_z.resize(self.num_nodes, 0.0);

        self.pos_in_x.copy_to(target_x)?;
        self.pos_in_y.copy_to(target_y)?;
        self.pos_in_z.copy_to(target_z)?;

        self.transfer_events[event_idx].record(&self.transfer_stream)?;

        self.pos_transfer_pending = true;
        Ok(())
    }

    pub fn wait_for_download_positions(&mut self) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        if !self.pos_transfer_pending {
            return Ok(self.get_current_position_buffer());
        }

        let event_idx = if !self.current_pos_buffer { 1 } else { 0 };
        self.transfer_events[event_idx].synchronize()?;

        self.pos_transfer_pending = false;
        self.current_pos_buffer = !self.current_pos_buffer;

        Ok(self.get_current_position_buffer())
    }

    pub fn start_async_download_velocities(&mut self) -> Result<()> {
        if self.vel_transfer_pending {
            return Ok(());
        }

        let target_buffer = !self.current_vel_buffer;
        let event_idx = if target_buffer { 1 } else { 0 };

        let (target_x, target_y, target_z) = if target_buffer {
            (
                &mut self.host_vel_buffer_b.0,
                &mut self.host_vel_buffer_b.1,
                &mut self.host_vel_buffer_b.2,
            )
        } else {
            (
                &mut self.host_vel_buffer_a.0,
                &mut self.host_vel_buffer_a.1,
                &mut self.host_vel_buffer_a.2,
            )
        };

        target_x.resize(self.num_nodes, 0.0);
        target_y.resize(self.num_nodes, 0.0);
        target_z.resize(self.num_nodes, 0.0);

        self.vel_in_x.copy_to(target_x)?;
        self.vel_in_y.copy_to(target_y)?;
        self.vel_in_z.copy_to(target_z)?;

        self.transfer_events[event_idx].record(&self.transfer_stream)?;

        self.vel_transfer_pending = true;
        Ok(())
    }

    pub fn wait_for_download_velocities(&mut self) -> Result<(Vec<f32>, Vec<f32>, Vec<f32>)> {
        if !self.vel_transfer_pending {
            return Ok(self.get_current_velocity_buffer());
        }

        let event_idx = if !self.current_vel_buffer { 1 } else { 0 };
        self.transfer_events[event_idx].synchronize()?;

        self.vel_transfer_pending = false;
        self.current_vel_buffer = !self.current_vel_buffer;

        Ok(self.get_current_velocity_buffer())
    }
}
