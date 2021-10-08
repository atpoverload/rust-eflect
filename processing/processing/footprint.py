""" Methods that turn EflectDataSets into EflectFootprints. """

from processing.accounting import account_jiffies, account_rapl_energy, align_yappi_methods

def compute_footprint(data):
    """ Produces an energy footprint from the data set. """
    activity = account_jiffies(data.task, data.cpu)
    energy = account_rapl_energy(activity, data.rapl)

    return energy
