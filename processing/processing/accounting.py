""" Methods that account activity and energy using pandas. """

import os

import numpy as np
import pandas as pd

from processing.preprocessing import cpu_samples_to_df
from processing.preprocessing import task_samples_to_df
from processing.preprocessing import rapl_samples_to_df

# TODO(timur): find out if there's a general conversion formula
DOMAIN_CONVERSION = lambda x: 0 if int(x) < 20 else 1

def account_jiffies(task, cpu):
    """ Returns the ratio of the jiffies with a correction for overaccounting. """
    task = task_samples_to_df(task)
    cpu = cpu_samples_to_df(cpu)
    return (task / cpu.replace(0, 1)).replace(np.inf, 1).clip(0, 1)

def account_rapl_energy(activity, rapl):
    """ Returns the product of the energy and the cpu-aligned activity data. """
    activity = activity.reset_index()
    activity['socket'] = activity.cpu.apply(DOMAIN_CONVERSION)
    activity = activity.set_index(['timestamp', 'id', 'socket'])[0]

    rapl = rapl_samples_to_df(rapl)

    # TODO(timur): we should just be able to take the product but the axis
    #   misalignment causes it to fail sometimes
    try:
        df = rapl * activity
    except:
        activity = activity.reset_index()
        rapl_energy = rapl_energy.reset_index()
        df = pd.merge(activity, rapl_energy, on=['timestamp', 'socket'])
        df[0] = df['0_x'] * df['0_y']
        df = df.set_index(['timestamp', 'id', 'component', 'socket'])[0]

    return df.reset_index().set_index(['timestamp', 'id', 'component', 'socket'])

def align_yappi_methods(energy, yappi_methods):
    """ Aligns yappi traces to timestamp-id pairs. """
    energy = energy.reset_index()
    energy['name'] = energy.id.str.split('-').str[1]
    energy.id = energy.id.str.split('-').str[0].replace(np.nan, 0).astype(int)

    energy = energy.groupby(['timestamp', 'id', 'name', 'component'])[0].sum() * yappi_methods
    energy = energy.groupby(['timestamp', 'id', 'name', 'component', 'stack_trace']).sum().sort_values(ascending=False)

    return energy
